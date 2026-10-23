use audiosync_audio::{OpusEncoderPipeline, SyntheticPcmSource};
use audiosync_codec::{FRAME_PCM_SAMPLES, OpusDecoder};
use audiosync_core::{AudioPacket, JitterBuffer, PushResult, SessionId};
use audiosync_session::{
    HostSession, ListenerSession, ListenerState, SessionAction, SessionMetrics,
};
use audiosync_transport::{ControlMessage, SessionToken, UdpTransport};
use std::net::SocketAddr;
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct AudioSimulationMetrics {
    offered_packets: u64,
    network_dropped_packets: u64,
    delivered_packets: u64,
    duplicate_packets: u64,
    skipped_packets: u64,
    concealed_frames: u64,
    rendered_frames: u64,
}

fn main() {
    let session_metrics = run_session_scenario();
    let audio_metrics = run_audio_scenario();
    run_udp_control_round_trip();

    println!(
        "SyncIt simulator PASS: active listeners {}, reconnects {}, timeouts {}, rendered {} frames, concealed {} frames, skipped {} packets, offset {} µs, RTT {} µs",
        session_metrics.active_listeners,
        session_metrics.reconnects,
        session_metrics.timeouts,
        audio_metrics.rendered_frames,
        audio_metrics.concealed_frames,
        audio_metrics.skipped_packets,
        25_000,
        10,
    );
}

fn run_session_scenario() -> SessionMetrics {
    let session_id: SessionId = [0x2A; 16];
    let token = SessionToken::new([0x7F; 16]);
    let host_endpoint = "127.0.0.1:41000"
        .parse::<SocketAddr>()
        .expect("host address is valid");
    let listener_endpoint = "127.0.0.1:41001"
        .parse::<SocketAddr>()
        .expect("listener address is valid");
    let started = Instant::now();

    let mut host = HostSession::new(session_id, token, 5);
    let mut listener = ListenerSession::new(session_id, token, host_endpoint);

    let join_message = outbound_message(listener.join_action());
    let join_actions = host.handle_message(listener_endpoint, join_message, started);
    deliver_to_listener(&mut listener, host_endpoint, join_actions, None);
    assert_eq!(listener.state(), ListenerState::PendingApproval);

    let listener_id = host
        .listener_id_for(listener_endpoint)
        .expect("host assigned a listener ID");
    let approval_actions = host
        .approve_listener(listener_id)
        .expect("host approves the pending listener");
    deliver_to_listener(&mut listener, host_endpoint, approval_actions, None);
    assert_eq!(listener.state(), ListenerState::Active);

    let clock_request = listener
        .clock_sync_action(0)
        .expect("active listener requests clock sync");
    let clock_request_message = outbound_message(clock_request);
    let client_sent_micros = match clock_request_message {
        ControlMessage::ClockSyncRequest {
            client_sent_micros, ..
        } => client_sent_micros,
        _ => unreachable!("clock action creates a clock request"),
    };
    host.handle_message(
        listener_endpoint,
        ControlMessage::ClockSyncRequest {
            session_id,
            client_sent_micros,
        },
        started + Duration::from_micros(2),
    );
    let clock_response = host
        .respond_to_clock_sync(
            listener_endpoint,
            client_sent_micros,
            25_005,
            25_006,
            started + Duration::from_micros(3),
        )
        .expect("host responds to clock sync");
    deliver_to_listener(&mut listener, host_endpoint, clock_response, Some(11));
    assert_eq!(listener.clock_offset_micros(), Some(25_000));
    assert_eq!(listener.best_round_trip_micros(), Some(10));

    let heartbeat = listener
        .heartbeat_action()
        .expect("active listener sends heartbeat");
    host.handle_message(
        listener_endpoint,
        outbound_message(heartbeat),
        started + Duration::from_micros(4),
    );

    let timeout_actions =
        host.expire_stale(started + Duration::from_secs(2), Duration::from_secs(1));
    deliver_to_listener(&mut listener, host_endpoint, timeout_actions, None);
    assert_eq!(listener.state(), ListenerState::Removed);

    let reconnect_actions = host.handle_message(
        listener_endpoint,
        outbound_message(listener.join_action()),
        started + Duration::from_secs(3),
    );
    deliver_to_listener(&mut listener, host_endpoint, reconnect_actions, None);
    assert_eq!(host.listener_id_for(listener_endpoint), Some(listener_id));
    assert_eq!(listener.state(), ListenerState::PendingApproval);

    let reapproval_actions = host
        .approve_listener(listener_id)
        .expect("reconnected listener requires approval");
    deliver_to_listener(&mut listener, host_endpoint, reapproval_actions, None);
    assert_eq!(listener.state(), ListenerState::Active);

    let metrics = host.metrics().clone();
    assert_eq!(metrics.join_requests, 2);
    assert_eq!(metrics.join_accepted, 2);
    assert_eq!(metrics.approvals, 2);
    assert_eq!(metrics.clock_sync_requests, 1);
    assert_eq!(metrics.clock_sync_responses, 1);
    assert_eq!(metrics.heartbeats, 1);
    assert_eq!(metrics.reconnects, 1);
    assert_eq!(metrics.timeouts, 1);
    assert_eq!(metrics.active_listeners, 1);
    metrics
}

fn deliver_to_listener(
    listener: &mut ListenerSession,
    host_endpoint: SocketAddr,
    actions: Vec<SessionAction>,
    client_received_micros: Option<i64>,
) {
    for action in actions {
        let SessionAction::Outbound { message, .. } = action else {
            continue;
        };
        listener
            .handle_message(host_endpoint, message, client_received_micros)
            .expect("simulated host message is valid");
    }
}

fn outbound_message(action: SessionAction) -> ControlMessage {
    match action {
        SessionAction::Outbound { message, .. } => message,
        SessionAction::Event(_) => unreachable!("simulated action should be outbound"),
    }
}

fn run_audio_scenario() -> AudioSimulationMetrics {
    let session_id: SessionId = [0x2A; 16];
    let mut buffer = JitterBuffer::new(8);
    buffer.start_at(0);
    let source = SyntheticPcmSource::new(220.0);
    let mut encoder = OpusEncoderPipeline::new(source).expect("synthetic source should initialize");
    let mut decoder = OpusDecoder::new(encoder.config()).expect("decoder should initialize");
    let mut metrics = AudioSimulationMetrics::default();

    // The order models Wi-Fi reordering. Sequence 2 is lost and sequence 3 is
    // duplicated before the scheduler reaches the missing-frame deadline.
    for sequence in [2_u32, 0, 1, 3, 3, 4] {
        metrics.offered_packets += 1;
        if sequence == 2 {
            metrics.network_dropped_packets += 1;
            continue;
        }

        let opus_payload = encoder
            .next_packet()
            .expect("synthetic PCM frame should encode");
        let packet = AudioPacket::new(
            session_id,
            sequence,
            u64::from(sequence) * 20_000,
            opus_payload,
        );
        let encoded = packet.encode().expect("simulated packet is valid");
        let decoded = AudioPacket::decode(&encoded).expect("simulated packet is valid");
        metrics.delivered_packets += 1;
        match buffer.push(decoded.sequence, decoded.payload) {
            PushResult::Duplicate => metrics.duplicate_packets += 1,
            PushResult::Inserted => {}
            PushResult::TooLate | PushResult::CapacityDrop => {
                panic!("unexpected packet drop in deterministic audio scenario")
            }
        }
    }

    let mut rendered_sequences = Vec::new();
    while let Some(encoded_frame) = buffer.pop_next() {
        let decoded_frame = decoder
            .decode(&encoded_frame)
            .expect("received Opus frame should decode");
        assert_eq!(decoded_frame.len(), FRAME_PCM_SAMPLES);
        rendered_sequences.push(rendered_sequences.len() as u8);
    }
    assert_eq!(rendered_sequences.len(), 2);
    assert!(buffer.skip_missing());
    metrics.skipped_packets = buffer.skipped_packets();
    let concealed_frame = decoder
        .decode_loss()
        .expect("lost Opus frame should be concealed");
    assert_eq!(concealed_frame.len(), FRAME_PCM_SAMPLES);
    metrics.concealed_frames += 1;
    while let Some(encoded_frame) = buffer.pop_next() {
        let decoded_frame = decoder
            .decode(&encoded_frame)
            .expect("received Opus frame should decode");
        assert_eq!(decoded_frame.len(), FRAME_PCM_SAMPLES);
        rendered_sequences.push(rendered_sequences.len() as u8);
    }
    assert_eq!(rendered_sequences.len(), 4);
    metrics.rendered_frames = rendered_sequences.len() as u64;
    assert_eq!(metrics.network_dropped_packets, 1);
    assert_eq!(metrics.duplicate_packets, 1);
    metrics
}

fn run_udp_control_round_trip() {
    let receiver = UdpTransport::bind("127.0.0.1:0").expect("receiver should bind");
    receiver
        .set_read_timeout(Some(Duration::from_millis(250)))
        .expect("receiver timeout should be configured");
    let sender = UdpTransport::bind("127.0.0.1:0").expect("sender should bind");
    let message = ControlMessage::Heartbeat {
        session_id: [0x2A; 16],
    };
    let encoded = message.encode().expect("heartbeat should encode");
    sender
        .send_to(
            &encoded,
            receiver.local_addr().expect("receiver has address"),
        )
        .expect("UDP datagram should send");

    let mut received = [0_u8; 1_024];
    let (length, source) = receiver
        .recv_from(&mut received)
        .expect("UDP datagram should arrive");
    assert_eq!(source, sender.local_addr().expect("sender has address"));
    assert_eq!(
        ControlMessage::decode(&received[..length]).expect("datagram should decode"),
        message
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_audio_pipeline_remains_a_regression_scenario() {
        let metrics = run_audio_scenario();

        assert_eq!(metrics.rendered_frames, 4);
        assert_eq!(metrics.concealed_frames, 1);
        assert_eq!(metrics.network_dropped_packets, 1);
        assert_eq!(metrics.duplicate_packets, 1);
        assert_eq!(metrics.skipped_packets, 1);
    }
}

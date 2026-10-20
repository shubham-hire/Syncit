use audiosync_core::{AudioPacket, ClockEstimator, ClockSample, JitterBuffer, SessionId};

fn main() {
    let session_id: SessionId = [0x2A; 16];
    let mut buffer = JitterBuffer::new(8);
    buffer.start_at(0);

    // Deliver frames out of order, as a real Wi-Fi path can do.
    for sequence in [2_u32, 0, 1, 4, 3] {
        let packet = AudioPacket::new(
            session_id,
            sequence,
            u64::from(sequence) * 20_000,
            vec![sequence as u8, 0xA5],
        );
        let encoded = packet.encode().expect("simulated packet is valid");
        let decoded = AudioPacket::decode(&encoded).expect("simulated packet is valid");
        buffer.push(decoded.sequence, decoded.payload);
    }

    let mut rendered = Vec::new();
    while let Some(frame) = buffer.pop_next() {
        rendered.push(frame[0]);
    }

    assert_eq!(rendered, vec![0, 1, 2, 3, 4]);

    let mut clock = ClockEstimator::default();
    assert!(clock.observe(ClockSample {
        client_sent_micros: 0,
        host_received_micros: 25_005,
        host_sent_micros: 25_006,
        client_received_micros: 11,
    }));

    println!(
        "AudioSync simulator PASS: rendered {} frames, host offset {} µs, RTT {} µs",
        rendered.len(),
        clock.offset_micros().expect("clock sample was accepted"),
        clock
            .best_round_trip_micros()
            .expect("clock sample was accepted")
    );
}

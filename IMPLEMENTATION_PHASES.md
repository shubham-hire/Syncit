# AudioSync implementation phases

Last updated: October 24, 2026

This document records the implementation phases for the Windows-host and
Android-listener validation path. It distinguishes reusable engine primitives
from a real host/listener runtime so that a passing simulator is not mistaken
for a completed hotspot test.

## Status legend

- **Complete:** implemented and covered by a check or recorded hardware run.
- **In progress:** the reusable building block exists, but the real user flow
  still needs integration or hardware validation.
- **Not started:** no implementation has been added yet.

## Phase 0: Protocol and codec contracts — Complete

The platform-neutral foundation is in place:

- Fixed 48 kHz, mono, signed 16-bit PCM.
- 20 ms frames containing 960 samples.
- Stateful Opus encoding and decoding.
- Versioned `AudioPacket` serialization with session ID, sequence and timestamp.
- Clock-offset estimation.
- Bounded reorder-aware jitter buffering.
- Session and control-message state machines.

Primary locations:

```text
backend/native/codec/
backend/native/core/
backend/native/transport/src/control.rs
backend/native/session/
```

## Phase 1: Windows system-audio capture — Complete

The Windows adapter captures the default Multimedia render endpoint through
shared-mode WASAPI loopback, converts stereo float audio to the fixed mono PCM
contract, and supplies codec-sized frames.

The checked-in smoke example validates:

```text
WASAPI loopback
  → 48 kHz mono PCM
  → Opus encode
  → Opus decode
  → 960-sample frame validation
```

Native Windows results are recorded in
`backend/native/audio/WINDOWS_WASAPI_TESTING.md` for:

- Default laptop speakers.
- Wired headphones.
- A Bluetooth headset.

All three 500-frame runs passed with 500 encoded packets, 500 decoded packets,
500 non-silent frames, and zero WASAPI timeouts.

## Phase 2: Platform-neutral host packet pump — Complete

`AudioPacketPump<S>` now connects any `PcmSource` to the versioned
`AudioPacket` format without depending on Windows APIs or sockets.

Implemented behavior:

- Owns one stateful `OpusEncoderPipeline` for the stream lifetime.
- Produces one `AudioPacket` per successful PCM frame.
- Starts sequence numbering at zero and advances only after successful encoding.
- Uses `capture_start_micros + frame_index * 20_000` for the current codec
  contract.
- Preserves the same session ID on every packet.
- Reports source/codec failures and timestamp overflow as typed errors.

Implementation and tests:

```text
backend/native/audio/src/lib.rs
backend/native/audio/Cargo.toml
```

The packet pump is a reusable library component. It is not yet a long-running
host thread and does not decide which listeners are active.

## Phase 3: UDP audio fan-out — Complete at transport/simulation level

`AudioPacketFanout` now encodes one packet once and sends it to every supplied
listener endpoint.

Implemented behavior:

- Sends the same sequence and timestamp to every destination.
- Continues sending after an individual destination fails.
- Reports attempted, successful, failed, and byte counts.
- Preserves packet encoding errors as the return value.

Implementation and tests:

```text
backend/native/transport/src/audio.rs
backend/native/transport/src/lib.rs
```

The simulator now proves that three synthetic packets can reach two local UDP
listeners and decode back into the same `AudioPacket` values.

This phase does not yet connect fan-out destinations to the live
`HostSession` active-listener map.

## Phase 4: Session-aware host runtime — In progress (local runtime complete)

`audiosync-runtime` now owns the local control/audio orchestration needed for a
host prototype:

1. Bind the host control and audio sockets.
2. Feed decoded control datagrams into `HostSession`.
3. Encode and send returned control actions.
4. Approve listeners through the host UI/runtime.
5. Collect only active listener endpoints.
6. Call `AudioPacketPump::next_packet()` every 20 ms.
7. Pass each packet to `AudioPacketFanout`.
8. Expire stale listeners and stop sending to them.

The existing session layer intentionally does not own sockets, threads or
clocks, so this phase belongs in a host runtime/orchestration layer rather
than in `audiosync-session`. The runtime is covered by a local UDP test that
joins, approves, clock-syncs, and sends one synthetic audio packet to a
listener.

The scheduler now exists: `HostLoop` (`backend/native/runtime/src/host_loop.rs`)
owns the runtime on one thread, drives the 20 ms audio cadence from a
drift-corrected anchor, dispatches control between ticks with real monotonic
clock-sync timestamps, expires stale listeners, and executes approve/remove/
end commands from a channel. The remaining work is Windows hotspot validation
and UI approval wiring.

Implementation:

```text
backend/native/runtime/Cargo.toml
backend/native/runtime/src/lib.rs
backend/native/runtime/src/host_loop.rs
```

## Phase 5: Reference listener pipeline — In progress (local receiver complete)

The reusable listener pipeline and socket wrapper now handle:

```text
UDP datagram
  → AudioPacket validation
  → sequence/timestamp tracking
  → JitterBuffer
  → OpusDecoder
  → live PCM sink
```

It must expose packet loss, late packets, duplicates, jitter-buffer underruns,
clock offset, and concealment metrics. It must not write recordings by default.

The current implementation validates packets, reorders them, decodes ready
frames, and explicitly produces Opus loss-concealment frames after the caller's
playback deadline. `ListenerRuntime` owns a UDP socket and returns decoded PCM
frames to its caller. `PlaybackScheduler`
(`backend/native/runtime/src/playback.rs`) now paces those frames on a
wall-clock timeline (configurable pre-roll, loss grace, and manual listener
delay) while keeping the heartbeat and clock-sync cadence. The `audiosync-host`
and `audiosync-listener` binaries (`backend/native/apps/`) wrap both loops and
play through CPAL or a null sink; a 5-second local run streams 250 frames at
the exact 50 frames/s cadence with zero concealment. The Android foreground
service remains pending.

## Phase 6: One real hotspot listener — Not started

Validate one receiver over Windows Mobile Hotspot before adding multiple
listeners:

1. Connect the receiver to the host hotspot.
2. Confirm host-to-receiver UDP reachability.
3. Check Windows Firewall and hotspot client isolation.
4. Run synthetic audio over the real hotspot.
5. Replace the synthetic source with WASAPI capture.
6. Test wired output and Bluetooth output separately.

The first target remains a Windows host with an Android listener. The repository
does not yet contain the Android foreground playback service.

## Phase 7: Multiple listeners and timing calibration — Not started

After one listener is stable:

- Fan out to two and then five listeners.
- Verify identical sequence/timestamp streams.
- Confirm one slow or disconnected listener does not stop others.
- Measure capture, network, jitter-buffer, wired-output and Bluetooth latency.
- Add per-listener delay adjustment.

Clock synchronization does not remove Bluetooth hardware latency; manual delay
calibration remains part of the prototype requirements.

## Phase 8: Flutter, Android and operational hardening — Not started

Remaining integration and hardening work includes:

- Android foreground playback service.
- Flutter FFI bridge to the Rust engine.
- Join/discovery flow without manual IP entry.
- Reconnect and screen-lock behavior.
- Malformed-packet and protocol-version handling.
- Session traffic authentication.
- Windows Firewall and hotspot diagnostics.
- Device compatibility and latency test matrix.
- Sustained 30-minute playback validation.

## Robustness, authentication, and tooling (October 25, 2026)

- Listener watchdog: sustained host silence (default 3 s) marks the session
  `TimedOut`, then the playback scheduler rejoins automatically on a capped
  backoff (PRD recovery target: ~2 s). Covered by runtime tests.
- Protocol version 2: every audio packet now carries a 16-byte truncated
  HMAC-SHA256 tag keyed by the session token (`sha2` + `hmac` + `subtle`).
  The pump signs, listeners verify in constant time, and forged datagrams
  are counted (`unauthenticated_packets`) and dropped without interrupting
  playback.
- Calibration tooling: `audiosync-calibrate` cross-correlates two WAV
  recordings of the same playback into a per-listener `--delay-ms`
  suggestion. `audiosync-listener --csv FILE` records per-second soak
  metrics.
- `HOTSPOT_VALIDATION.md` documents the Phase 6 hardware runbook (firewall,
  addressing, run matrix, go/no-go).
- `audiosync-ffi`: a narrow C ABI over the runtime (host start/command,
  listener join/pump/leave, JSON in/out) with a full C-ABI round-trip test.
  Dart bindings and the `EngineController` live in
  `frontend/flutter-app/lib/engine/` with mock fallback; page wiring awaits a
  Flutter-capable machine (see the engine README).

## Current boundary

The repository currently proves:

```text
real Windows capture → Opus round trip
synthetic source → AudioPacketPump
AudioPacket → UDP fan-out → two local UDP listeners
synthetic source → local HostRuntime → ListenerAudioPipeline → decoded PCM
```

It does not yet prove:

```text
real Windows capture → hotspot → Android listener → Bluetooth earbuds
```

That end-to-end result begins in Phase 4 and requires the receiver runtime in
Phases 5 and 6.

## Validation evidence from October 24, 2026

The current Rust workspace passed:

```text
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build --workspace --locked
cargo run -p audiosync-sim --locked
```

The simulator reported:

```text
SyncIt simulator PASS: active listeners 1, reconnects 1, timeouts 1,
rendered 4 frames, concealed 1 frames, skipped 1 packets,
offset 25000 µs, RTT 10 µs
```

The reusable runtime tests also pass a 500-packet local UDP-to-PCM path. A
post-change live WASAPI recheck in the current environment did not have a
confirmed active playback context and stopped at frame 1 with the documented
one-second capture timeout; the three earlier native hardware runs remain the
authoritative speaker, wired-headphone, and Bluetooth-headset results.

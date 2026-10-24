# AudioSync backend and native engine

The backend area contains the platform-neutral Rust engine, local UDP transport,
host/listener session state, and an offline simulator. It does not contain a
cloud server or hosted API: AudioSync is local-first and the host device owns the
session.

```text
native/core/       Protocol, clock, and jitter-buffer primitives
native/codec/      Fixed 48 kHz mono Opus frame boundary and PLC
native/audio/      PCM source boundary, synthetic source, and Windows WASAPI loopback
native/transport/  UDP sockets and versioned control messages
native/session/    Deterministic host/listener state machines
native/sim/        Offline host/listener and packet-loss simulation
```

The root Cargo workspace owns all six crates. Run Rust commands from the
repository root:

```bash
cargo test --workspace --locked
cargo run -p audiosync-sim --locked
```

The first backend milestone deliberately keeps the session state machine
independent from sockets and clocks. A caller feeds decoded control messages
into `audiosync-session`, sends returned outbound messages through
`audiosync-transport`, and supplies monotonic timestamps for heartbeats, clock
sync, and timeout decisions.

Windows capture now lives in the explicit `native/audio` adapter rather than in
the protocol core or session state machine. Android playback and additional
platform audio adapters should follow the same boundary.

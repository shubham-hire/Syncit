# SyncIt backend and native engine

The backend area contains the platform-neutral Rust engine, local UDP transport,
host/listener session state, and an offline simulator. It does not contain a
cloud server or hosted API: SyncIt is local-first and the host device owns the
session.

```text
native/core/       Protocol, clock, and jitter-buffer primitives
native/codec/      Fixed 48 kHz mono Opus frame boundary and PLC
native/transport/  UDP sockets and versioned control messages
native/session/    Deterministic host/listener state machines
native/sim/        Offline host/listener and packet-loss simulation
```

The root Cargo workspace owns all five crates. Run Rust commands from the
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

Future Windows capture, Android playback, codec, and platform audio adapters
should be added as explicit native modules rather than putting platform code in
the protocol core or session state machine.

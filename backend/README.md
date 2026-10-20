# SyncIt backend and native engine

The backend area currently contains the platform-neutral Rust engine and an
offline simulator. It does not yet contain a network server or hosted API.

```text
native/core/  Protocol, clock, and jitter-buffer primitives
native/sim/   Offline host/listener simulation
```

The root Cargo workspace owns both crates. Run Rust commands from the
repository root:

```bash
cargo test --workspace --locked
cargo run -p audiosync-sim --locked
```

Future Windows capture, Android playback, and local-network transport adapters
should be added as explicit native modules rather than putting platform code in
the protocol core.

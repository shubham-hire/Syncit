# Native engine workspace

This directory is managed by the root `Cargo.toml` workspace.

```text
core/       Platform-neutral packet, timing, and jitter-buffer primitives
codec/      Fixed 48 kHz mono Opus frame boundary
audio/      PCM source contract, synthetic source, and Windows WASAPI loopback adapter
transport/  UDP wrapper and versioned session-control wire messages
session/    Host/listener state and clock-sync behavior
sim/        Offline host/listener and packet-loss simulator
```

Keep operating-system adapters separate from `core/`. The `audio` crate owns the
PCM source boundary and the Windows WASAPI loopback implementation; it converts
native stereo float samples into the fixed mono PCM frames accepted by the
codec. The protocol, transport, and session crates remain usable without
Windows or Android APIs.

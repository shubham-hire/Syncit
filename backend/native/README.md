# Native engine workspace

This directory is managed by the root `Cargo.toml` workspace.

```text
core/       Platform-neutral packet, timing, and jitter-buffer primitives
codec/      Fixed 48 kHz mono Opus frame boundary
transport/  UDP wrapper and versioned session-control wire messages
session/    Host/listener state and clock-sync behavior
sim/        Offline host/listener and packet-loss simulator
```

Keep operating-system adapters separate from `core/`. When implementation
starts, platform-specific capture, codec, and playback adapters can be added as
focused crates or modules without making the protocol or session state depend
on Windows or Android APIs. The transport and session crates must remain usable
with a caller-owned socket loop and caller-owned monotonic clock.

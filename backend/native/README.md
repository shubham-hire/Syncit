# Native engine workspace

This directory is managed by the root `Cargo.toml` workspace.

```text
core/   Platform-neutral packet, timing, and jitter-buffer primitives
sim/    Offline host/listener simulator
```

Keep operating-system adapters separate from `core/`. When implementation
starts, platform-specific capture, transport, codec, and playback adapters can
be added as focused crates or modules without making the protocol primitives
depend on Windows or Android APIs.

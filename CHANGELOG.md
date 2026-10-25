# Changelog

## October 25, 2026 — automation session

Everything below was implemented, gated (`cargo fmt --check`,
`clippy -D warnings`, `cargo test --workspace --locked`, `cargo build`,
simulator PASS — 68 tests), and pushed to `main`.

### Streaming prototypes (Milestones 0–1)

- **`HostLoop`** (`backend/native/runtime/src/host_loop.rs`): single-threaded
  scheduler owning the host runtime — drift-corrected 20 ms audio cadence,
  control dispatch with monotonic clock-sync timestamps, stale-listener
  expiry, and an approve/remove/end command channel for future UIs.
- **`PlaybackScheduler`** (`playback.rs`): listener wall-clock pacing with
  configurable pre-roll, loss grace, and manual delay; heartbeat and
  clock-sync cadence; `PcmSink` trait with `NullSink` for tests.
- **`audiosync-apps` crate**: `audiosync-host` (WASAPI on Windows, synthetic
  elsewhere; prints a paste-ready listener command) and `audiosync-listener`
  (CPAL output, `--null-output`, per-second metrics, `--csv` soak logging).
  `make host` / `make listener`. Verified live on loopback at exact cadence.

### Robustness and authenticated audio (Milestone 3)

- **Listener watchdog**: 3 s of host silence marks the session `TimedOut`
  (new `ListenerSession::mark_timed_out`; terminal host decisions preserved),
  then auto-rejoins on a backoff capped at 2 s (PRD recovery target).
- **Protocol version 2**: every audio packet carries a 16-byte truncated
  HMAC-SHA256 tag keyed by the session token (`sha2` + `hmac` + `subtle` in
  `audiosync-core`). The pump signs; listeners verify in constant time;
  forged datagrams are counted (`unauthenticated_packets`) and dropped
  without interrupting playback. v1 packets are rejected — both sides ship
  together.

### Calibration tooling and Phase 6 runbook (Milestones 4-tooling and 2-prep)

- `audiosync-calibrate`: WAV parsing (PCM16) plus envelope
  cross-correlation; reports which listener leads and the `--delay-ms`
  adjustment. Unit-tested against known 60 ms offsets. `make calibrate`.
- `audiosync-listener --csv FILE`: per-second soak metrics rows.
- `HOTSPOT_VALIDATION.md`: firewall PowerShell, hotspot addressing, a
  6-step run matrix, recording template, and go/no-go criteria.

### FFI bridge (Milestone 5, Rust side complete)

- **`audiosync-ffi` crate** (`cdylib`/`staticlib`): five C functions over the
  engine — host start/command, listener join/pump/leave — exchanging JSON;
  slot-based handles, bounded event logs. Tested end-to-end through the C
  boundary (start → join → active → 495 frames → end).
- **Dart side** (`frontend/flutter-app/lib/engine/`): `dart:ffi` bindings and
  an `EngineController` with automatic mock fallback, so widget tests are
  unaffected. Page wiring is documented in `engine/README.md` but awaits a
  Flutter-capable machine (`flutter analyze` + `flutter test` first).

### Landing page (branch merged this session)

- Refined 3D models (PBR clearcoat materials, PMREM environment, ACES tone
  mapping; lathed earcup shells, detailed earbuds).
- Scroll-reveal choreography (`use-reveal.ts`), load animations, and a
  footer-exit so the rig dissolves before sliding under the footer.

### Infrastructure and housekeeping

- **GitHub Actions CI** on every push: Linux full gate, landing-page
  lint/test/build, and a Windows job. Windows is **compile-only** by owner
  request (validates the WASAPI/FFI build; tests run on the dev PCs).
- SyncIt → AudioSync naming sweep across Rust, docs, and the Flutter app
  (GitHub URLs and the landing page excluded). Simulator prints
  `AudioSync simulator PASS`.
- `.zcode/` ignored; `ROADMAP.md` records the milestone plan.

### Fixes found by CI on first push

- The first Windows job caught a real bug: `wasapi`'s COM types are not
  `Send`, which broke `spawn_host_loop`'s bound in the Windows-only binaries.
  Fixed with a documented `unsafe impl Send for WasapiCapture` — sound
  because the capture initializes COM in the multi-threaded apartment and
  owns a single loop thread. Verified for the Windows target with an
  isolated cross-check probe.

## Earlier milestones

See `IMPLEMENTATION_PHASES.md` for the phase-by-phase record (protocol/codec
foundation, Windows WASAPI capture with hardware validation, packet pump,
UDP fan-out, host/listener runtimes).

# Changelog

## October 25, 2026 — Windows Flutter listener output

- Replaced the Flutter FFI listener's per-pump `NullSink` with a persistent
  Windows CPAL/WASAPI output path. Decoded PCM frames now remain connected to
  the default Windows output between Dart pump calls.
- Added `CpalChannelSink`, which owns CPAL's deliberately non-`Send` stream on
  a dedicated worker thread and exposes a bounded, `Send`-safe PCM bridge to
  the FFI handle table.
- Added unsigned 8-bit (`U8`) output conversion for Windows devices that
  advertise 8-bit PCM instead of float or signed 16-bit samples.
- Listener status now reports the active output label. Rust tests continue to
  use a deterministic null sink, while release Windows builds require a
  default output device and report a clear startup error if none is available.
- Volume and mute controls remain UI-only until native sink controls are
  connected.
- Added a host-side pending-approval banner and review dialog with Approve and
  Reject actions. The listener now presents `Waiting for host approval`
  instead of an opaque `PendingApproval` status while audio streaming is
  correctly paused.
- Fixed host event serialization: a trailing comma in the FFI event payload
  prevented Dart from parsing `ListenerJoined`, so pending listeners were not
  reaching the approval UI. The Dart parser also tolerates the legacy payload
  while deployed DLLs are refreshed.
- Added a host source selector for the Flutter engine page: WASAPI loopback is
  the default, while a generated synthetic tone can isolate transport and
  listener-output problems.
- Added host diagnostics for runtime errors/events and listener diagnostics for
  packet receipt, authentication, jitter-buffer drops, clock offset, sink
  failures, watchdog timeouts, and rejoin attempts.
- Rebases the first host audio packet timestamp when the first listener becomes
  active. A host can wait for approval long after startup; without this rebase,
  the listener treated the first live packets as stale, skipped their sequence
  numbers, and reported increasing late packets with an empty buffer.
- Added explicit same-PC WASAPI warnings. Running host and listener on the same
  default output intentionally produces the original audio plus a delayed
  loopback copy; the clean WASAPI test uses a separate listener device or
  output.

## October 25, 2026 — Windows fix + Milestone 5 wiring (local session)

Implemented and gated on the Windows development machine. Committed locally;
**not pushed yet**.

### Windows CI failure root-caused and fixed

- The remaining Windows-only compile error was E0308 in the two
  `#[cfg(windows)]` source-selection sites: the if/else arms produced
  `SyntheticPcmSource` vs `WasapiCapture` — two different concrete types.
  Linux CI stayed green because the `#[cfg(not(windows))]` fallback uses only
  the synthetic source. Files: `apps/src/bin/audiosync_host.rs` and
  `ffi/src/lib.rs`.
- Fix: `audiosync-audio` now implements `PcmSource for Box<S>` (blanket
  delegation, unit-tested), and both call sites type the selection as
  `Box<dyn PcmSource + Send>`, which satisfies `spawn_host_loop`'s
  `PcmSource + Send + 'static` bound (both sources are Send; the documented
  `unsafe impl Send for WasapiCapture` covers the WASAPI case).
- **Full gate green on Windows for the first time**: fmt, clippy
  `-D warnings`, all 68 tests (previously Windows ran compile-only),
  build, and `AudioSync simulator PASS`.

### Milestone 5 — Flutter pages wired to the engine

- Generated the platform scaffolding with Flutter 3.47.8
  (`flutter create --platforms=android,windows .`; `lib/` and `test/`
  untouched; `.fvmrc` bumped 3.47.7 → 3.47.8) and added `flutter_lints`.
- `host_session_page.dart` now calls `EngineController.create()`:
  engine mode starts the real WASAPI host (`autoApprove: false`), polls
  events, and drives the listener list/approve/remove through
  `HostEvent.parse`d `ListenerJoined`/`ListenerApproved`/`ListenerRemoved`
  /`ListenerTimedOut` events; mock mode keeps the exact prior behavior the
  widget tests assert.
- `listener_join_page.dart`: engine mode collects host address plus
  session/token hex, joins, and renders live state, rendered/concealed
  frames, buffer depth and RTT; the delay slider applies live via
  `pumpListener(delayMs:)`. Volume stays local until the platform output
  exists (Milestone 6).
- Fixed a latent Dart defect caught by first-ever `flutter analyze`:
  `package:ffi/ffi.dart show malloc` hid the `Utf8` type and extension
  methods `ffi_bindings.dart` itself uses — the bindings could never have
  compiled. Analyze is now clean (`flutter_lints`).
- Engine library discovery hardened: candidates are found by walking up
  from both the working directory and the script location to
  `target/release/` (workspace build) and the crate-local fallback, so
  `flutter run` and `dart run` both work; widget tests always run in mock
  mode (`FLUTTER_TEST` guard) so the suite stays deterministic on machines
  where the DLL exists.
- Verified end-to-end on Windows: `audiosync_ffi.dll` loads through the Dart
  bindings, a host session starts over real UDP, and the session/token hex
  return; `flutter build windows --debug` succeeds; `Makefile format-check`
  uses `dart format` (`flutter format` no longer exists).

### Follow-up — FFI lifecycle hardening

- Added a versioned `LeaveRequest` control message and host-side handling so a
  listener leaves cleanly instead of sending a duplicate join.
- Host command actions now retain approval/removal events, and ending a host
  loop stops its worker deterministically.
- Added `audiosync_ffi_host_stop`, explicit Dart host cleanup, listener slot
  release, and regression coverage for repeated host/listener handle reuse.
- Platform-neutral Rust, Flutter, and simulator checks pass; live WASAPI,
  Windows UI, hotspot, and Android playback validation remain hardware work.

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

### Current CI state (end of session)

- **Rust (Linux, full gate): passing** — fmt, clippy `-D warnings`, all 68
  tests, build, simulator.
- **Landing page (lint, test, build): passing.**
- **Rust (Windows, WASAPI compile check): still failing after the Send
  fix** — a further Windows-only compile error remains undiagnosed. The
  `Send` defect was verified fixed via a windows-msvc cross-check probe, so
  the remaining error is something else in the Windows build (the full
  error is visible in the Actions log; log download needs a browser
  session, which this automation did not have). The owner is fixing this
  next. Latest run: the "Build all binaries" step of the
  `Rust (Windows, WASAPI compile check)` job.
- Local Mac gate at HEAD `4f6a542`: fmt/clippy/tests/build/sim all green.

## Earlier milestones

See `IMPLEMENTATION_PHASES.md` for the phase-by-phase record (protocol/codec
foundation, Windows WASAPI capture with hardware validation, packet pump,
UDP fan-out, host/listener runtimes).

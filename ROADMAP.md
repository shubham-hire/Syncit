# AudioSync MVP roadmap

This file is the working plan agreed on October 25, 2026. Milestones 0 and 1
are complete. Update this file and `IMPLEMENTATION_PHASES.md` as work lands.

Hardware milestones (2, 4 fieldwork, 6 device validation) require the owner at
physical machines; everything else can be implemented and verified offline.

## Milestone 0 — Scheduler foundation — COMPLETE

CMake unlocked local full builds; `HostLoop` and `PlaybackScheduler` own the
20 ms host cadence and the listener wall-clock timeline; `PcmSink`/`NullSink`
abstract output. Committed on `backend-scheduler-apps`.

## Milestone 1 — CLI binaries — COMPLETE

`audiosync-host` and `audiosync-listener` in `backend/native/apps` with CPAL
output, `make host` / `make listener`, README instructions. Verified 250
frames / 5 s at exact cadence on loopback with zero concealment.

## Milestone 2 — Phase 6: first real hotspot listener — HARDWARE (owner)

1. Windows Mobile Hotspot on; note the hotspot-side host IP.
2. Firewall rule for the UDP port (PowerShell in `HOTSPOT_VALIDATION.md`).
3. `make host ARGS="--synthetic"` over Wi-Fi first; then real WASAPI.
4. Listener wired output, then Bluetooth, separately.
5. Record results in `HOTSPOT_VALIDATION.md`; tune `--buffer-ms` from jitter.
6. Go/no-go per PRD §12; fallback to an external AP if clients are isolated.

## Milestone 3 — Robustness + protocol v2 — COMPLETE (code)

- Listener watchdog: no host traffic for N seconds → `TimedOut` + auto-rejoin
  with capped backoff (PRD target: recovery within ~2 s).
- Protocol v2: 16-byte truncated HMAC-SHA256 tag over every audio packet,
  keyed by the session token (`sha2` + `hmac`, RustCrypto). Both sides verify;
  forged or replayed-session datagrams are dropped and counted.
- Clock offset/RTT reported continuously (drift visibility).

## Milestone 4 — Phase 7: multi-listener + calibration — TOOLING + HARDWARE

- Tooling (automatable): `audiosync-calibrate` cross-correlates two WAV
  recordings of the same playback into an inter-listener offset;
  `--csv` metrics on host/listener for 30-minute soaks.
- Fieldwork (owner): 2 → 5 listeners, kill-one-listener isolation, calibrate
  to the PRD's 20–50 ms, 30-minute soak with ≤1 interruption.

## Milestone 5 — Flutter bridge + Windows host UI — FFI COMPLETE; PAGE WIRING PENDING (needs Flutter SDK)

- `audiosync-ffi`: narrow C ABI over `HostRuntime`/`HostLoop` and
  `ListenerRuntime`/`PlaybackScheduler` (JSON event/command surface).
- Flutter: `flutter create --platforms=android,windows`; Dart `dart:ffi`
  controller with automatic mock fallback when the library is absent; real
  host session UI (token, listener list, approve/remove) and real listener
  join/playback. Mocks stay available for widget tests.

## Milestone 6 — Android listener + MVP demo — OWNER + GROUNDWORK

- Android foreground service playing the bridge PCM (AudioTrack/Oboe),
  screen-lock survival, battery guidance, QR join. Needs the Android SDK;
  design notes land with M5.

## Housekeeping track

- SyncIt → AudioSync naming sweep (Rust, Flutter, docs; landing page sits on
  its own branch and is handled there).
- `og:image` for the landing page (needs a 1200×630 asset — screenshot the
  hero) and a GitHub release with M1 binaries so the download CTA is real.
  Publishing is the owner's action.

## Working on the Windows machine — next steps

1. `git pull`, then `cargo build --workspace --locked`. This reproduces the
   open CI failure (Windows compile check) in seconds; the error message
   from the Actions log is at
   <https://github.com/shubham-hire/Syncit/actions/runs/40166961425>.
   The earlier `Send` defect is already fixed and probe-verified, so what
   remains is a different Windows-only compile error in the new crates
   (`apps`, `ffi`, `runtime` paths or the `cpal` Windows build are the
   candidates).
2. Isolation trick for Windows-only errors: a throwaway crate depending
   only on the suspect (e.g. `wasapi`) checked with
   `cargo check --target x86_64-pc-windows-msvc` skips the Opus/CMake build
   and compiles in seconds. On the Windows PC itself, plain
   `cargo check -p <crate>` is already fast.
3. Once it builds, run the full gate on the PC (CI is compile-only for
   Windows by owner choice): `cargo fmt --all -- --check`,
   `cargo clippy --workspace --all-targets --locked -- -D warnings`,
   `cargo test --workspace --locked`, `cargo run -p audiosync-sim --locked`.
4. Push; CI re-runs (Linux full gate + landing + Windows compile).
5. Then Milestone 2: follow `HOTSPOT_VALIDATION.md` (hotspot, firewall
   rule, `make host` / listener command with real WASAPI capture, wired
   then Bluetooth output, results into the table).

## Rules for unattended work

- Branch off the previous work; never commit on `main`; never push; never
  publish releases.
- Full gate before every commit:
  `cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo build --workspace --locked && cargo run -p audiosync-sim --locked`
- Local toolchain: `export PATH="$PWD/.tools/cargo/bin:$PATH" RUSTUP_HOME="$PWD/.tools/rustup"`.
- Flutter SDK (when present): `.tools/flutter-3.47.7/bin/flutter`.
- Record run state in `.zcode/automation-state.md` (local, gitignored).
- If a step needs an owner decision, note it and continue with the next
  automatable item.

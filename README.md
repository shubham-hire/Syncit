
# AudioSync

AudioSync is a student project for sharing one local audio session with
multiple non-Apple devices. The host enables a Wi-Fi hotspot; listeners join
that network and play the host audio through their own wired or Bluetooth
headphones.

The first implementation target is a **Windows host with Android listeners**.
The user interface is built with Flutter, while the timing, packet and audio
engine is being built as a platform-neutral Rust core. Apple devices are out
of scope for this project.

## Current milestone

The repository currently contains:

- A Rust workspace with versioned audio packet primitives.
- An NTP-style clock-offset estimator.
- A bounded, reorder-aware jitter buffer.
- A fixed 48 kHz mono Opus codec boundary with packet-loss concealment.
- A local UDP transport and host/listener session state machine.
- An offline host/listener simulator.
- A Flutter host/listener UI skeleton using mock session state, including mock
  host approval/removal and listener delay/volume controls.

Real WASAPI capture and Android foreground playback are the next integration
milestones.

## Repository layout

```text
frontend/            User-facing applications
  landing-page/      React/TanStack project landing page
  flutter-app/       Flutter host/listener application and widget tests
backend/             Native engine and simulation code
  native/core/       Shared Rust protocol and timing primitives
  native/codec/      Fixed 48 kHz mono Opus frame boundary
  native/transport/  UDP transport and session-control messages
  native/session/    Host/listener state and clock synchronization
  native/sim/        Offline host/listener and packet-loss simulation
PRD.md               Product requirements
Dockerfile           Reproducible Rust simulator image
Makefile             Common checks
```

## Prerequisites

### Rust core

The pinned Rust toolchain is `1.99.1`. Use a local Rust installation or the
Docker workflow below. The Opus crate builds its vendored native library through
CMake and a C compiler; the Dockerfile installs those build tools. The project
keeps `Cargo.lock` committed.

### Flutter UI

The pinned Flutter SDK is `3.47.7`, recorded in `.fvmrc`. The first Flutter
platform scaffolding still needs to be generated on a machine with Flutter.
`frontend/flutter-app/android`, `frontend/flutter-app/windows` and
`frontend/flutter-app/linux` are intentionally not checked in yet:

```bash
cd frontend/flutter-app
flutter create --platforms=android,windows,linux .
flutter pub get
```

The generated platform directories should be reviewed before committing.
Do not replace `lib/` or `test/` when generating the scaffolding.

## Run the Rust simulator

With Rust 1.99.1 available:

```bash
cargo run -p audiosync-sim --locked
```

Expected output includes:

```text
AudioSync simulator PASS
```

## Run the Flutter prototype

After generating the platform scaffolding:

```bash
cd frontend/flutter-app
flutter pub get
flutter run -d windows
```

The current UI demonstrates the host and listener flows with mock state. It
does not yet capture or play real audio.

## Tests and checks

The default test command is:

```bash
make test
```

This runs the Rust test suite and the Flutter widget tests. It fails clearly if
Flutter is missing rather than reporting an incomplete test run as success.
When Flutter is unavailable, run the available core checks with:

```bash
make test-core
```

Additional checks:

```bash
make format-check
make lint
make build
make test-landing
make lint-landing
make build-landing
```

Hardware validation is separate from automated tests and requires a Windows
machine, Android devices, a host hotspot and real audio outputs.

## Docker

The Docker image provides a reproducible Rust-core test and simulator path:

```bash
docker build -t audiosync:dev .
docker run --rm audiosync:dev
```

The Windows WASAPI and Android Bluetooth paths require their target platforms
and cannot be validated by the Linux simulator container alone.

Use FVM or an equivalent SDK manager to make the version in `.fvmrc` active
before running Flutter commands. The Makefile also accepts
`FLUTTER=/path/to/flutter` when a direct SDK path is preferred.

## Development sequence

1. Keep the protocol, session, and timing behavior tested in the offline simulator.
2. Add Opus encoding and a synthetic end-to-end audio pipeline.
3. Add Windows WASAPI loopback capture.
4. Add the Android foreground listener service.
5. Validate synchronization and manual per-listener delay adjustment.
6. Replace the Flutter mock session with the real engine bridge.

See `PRD.md` for product requirements and acceptance criteria.

## Run the landing page

The landing page is a Vite/TanStack application. It supports npm and Bun:

```bash
cd frontend/landing-page
npm install --legacy-peer-deps
npm run dev
```

Then open the URL printed by Vite, normally
[http://localhost:5173](http://localhost:5173).

Alternatively, use Bun with `bun install --frozen-lockfile` and `bun run dev`.
Do not open `frontend/landing-page/code.html`; that file is not part of the current
application.

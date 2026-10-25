# Flutter engine bridge (Milestone 5)

This directory contains the Dart side of the Rust engine bridge:

- `ffi_bindings.dart` — raw `dart:ffi` declarations for the six engine
  operations exported by `backend/native/ffi` (`audiosync_ffi`), including
  explicit host handle cleanup. The separate returned-string free function is
  handled internally by the bindings.
- `engine_controller.dart` — the state-machine wrapper the pages drive:
  start host, join listener, poll status, approve/remove/end, with automatic
  mock fallback whenever the native library is not loaded (widget tests and
  design-time previews run entirely in mock mode).

## Building the library

```bash
cargo build -p audiosync-ffi --release
```

Copy the artifact next to the executable (or keep one of the searched
relative paths):

| Platform | Artifact | Destination |
|----------|----------|-------------|
| Windows  | `target/release/audiosync_ffi.dll` | next to `audiosync_app.exe` |
| macOS    | `target/release/libaudiosync_ffi.dylib` | next to the app binary |
| Android  | `target/aarch64-linux-android/release/libaudiosync_ffi.so` | `android/app/src/main/jniLibs/arm64-v8a/` |

The workspace build puts the artifact in the repository-root `target/release/`
directory. The bindings try, in order: the process directory (bare name), the
repository-root `target/release/`, and `backend/native/ffi/target/release/` —
each both as-is and one directory up, so `flutter run` works from either the
repository root or `frontend/flutter-app`.

## Page wiring (done)

Both pages call `EngineController.create()` in their state, route through the
real engine when the library loads, and keep the exact mock behavior
otherwise (widget tests run entirely in mock mode):

- `host_session_page.dart` — `startHost(synthetic: false, autoApprove: false)`
  starts the real WASAPI host by default. Before starting, the page can switch
  to a generated synthetic tone for transport/output diagnostics. Host polling
  exposes running state, recent events, and runtime errors; the approve/remove
  buttons send `hostCommand` with the listener hex.
- `listener_join_page.dart` — engine mode collects host address plus
  session/token hex (the host prints them), `joinListener` joins, and
  `startListenerPolling` renders live state, rendered/concealed frames,
  received/inserted packets, invalid or unauthenticated packets, duplicate and
  late packets, buffer depth, clock offset, RTT, playback failures, recent
  events, and the active output label. On Windows, the FFI bridge keeps a
  persistent CPAL/WASAPI output stream alive between pump calls; the delay
  slider applies live through `pumpListener(delayMs: value)`. Volume and mute
  remain local UI controls until native sink controls are wired.

Both engine pages release their native host/listener handles from
`EngineController.dispose()`. Listener leave sends an explicit departure
request to the host before the local slot is released; host disposal sends an
orderly end command and then releases the host loop slot.

1. Generate platform scaffolding once (does not touch `lib/` or `test/`):

   ```bash
   cd frontend/flutter-app
   flutter create --platforms=android,windows .
   ```

The controller is intentionally UI-agnostic so the same calls serve the
Windows host app and the Android listener app.

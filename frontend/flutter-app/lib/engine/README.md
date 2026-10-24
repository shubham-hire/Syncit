# Flutter engine bridge (Milestone 5)

This directory contains the Dart side of the Rust engine bridge:

- `ffi_bindings.dart` — raw `dart:ffi` declarations for the five C functions
  exported by `backend/native/ffi` (`audiosync_ffi`).
- `engine_controller.dart` — the state-machine wrapper the pages drive:
  start host, join listener, poll status, approve/remove/end, with automatic
  mock fallback whenever the native library is not loaded (widget tests and
  design-time previews run entirely in mock mode).

## Building the library

```bash
cargo build -p audiosync-ffi --release
```

Copy the artifact next to the executable (or keep the searched relative path):

| Platform | Artifact | Destination |
|----------|----------|-------------|
| Windows  | `target/release/audiosync_ffi.dll` | next to `audiosync_app.exe` |
| macOS    | `target/release/libaudiosync_ffi.dylib` | next to the app binary |
| Android  | `target/aarch64-linux-android/release/libaudiosync_ffi.so` | `android/app/src/main/jniLibs/arm64-v8a/` |

The bindings search the process directory first, then
`backend/native/ffi/target/release/` (the layout used when running from the
repository root with `flutter run`).

## Wiring the pages

The pages currently use mock state (kept for widget tests). To go live:

1. Generate platform scaffolding once (does not touch `lib/` or `test/`):

   ```bash
   cd frontend/flutter-app
   flutter create --platforms=android,windows .
   ```

2. In `host_session_page.dart`, replace the mock session state with
   `EngineController.create()`:

   ```dart
   final _engine = EngineController.create();
   HostSessionInfo? _info;

   void _startSession() {
     final info = _engine.startHost(synthetic: false);
     if (info != null) setState(() => _info = info);
     _engine.startHostPolling((events) { /* update listener list */ });
   }
   ```

   Show `_info.sessionHex` / `_info.tokenHex` in the session-code card, map
   `ListenerJoined`/`ListenerRemoved` events to the listener list, and call
   `hostCommand('approve', listenerHex: ...)` / `('remove', ...)` from the
   existing buttons.

3. In `listener_join_page.dart`, call `joinListener(...)` with the host
   address and session/token hex, then `startListenerPolling` to update the
   delay/volume state and drive playback. `pumpListener(delayMs: value)`
   applies the delay slider live.

4. Playback audio on Android/Windows comes next (Milestone 6): the platform
   layer renders PCM from the engine. Until then the engine counts frames
   into a null sink so all controls and metrics are real.

The controller is intentionally UI-agnostic so the same calls serve the
Windows host app and the Android listener app.

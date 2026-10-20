# SyncIt Flutter application

This Flutter project contains the shared host/listener application UI.

```text
lib/
  main.dart                         Flutter entrypoint
  app.dart                          Material application shell and theme
  core/widgets/                     Widgets shared by features
  features/home/                    Role selection and project scope
  features/host/                    Host session flow and listener approval UI
  features/listener/                Listener join and playback-control UI
  features/session/                 Session constants and mock session boundary
test/                               Widget tests for the main flows
```

The current session and playback behavior is still mocked. Native capture,
transport, and playback services should be connected through feature/service
boundaries without putting real-time audio work in widgets.

## Commands

```bash
flutter pub get
flutter test
flutter analyze
```

Platform directories are generated from the repository root plan once the
pinned Flutter SDK is available:

```bash
flutter create --platforms=android,windows,linux .
```

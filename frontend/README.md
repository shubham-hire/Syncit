# SyncIt frontends

This directory contains the user-facing SyncIt applications:

- `landing-page/` — React, TanStack Start, Vite, and Three.js project site.
- `flutter-app/` — Flutter host/listener application UI.

The two applications are intentionally independent. Run their checks from
their own project directories rather than installing dependencies at this
directory level.

## Landing page

```bash
cd landing-page
npm install --legacy-peer-deps --include=optional
npm run dev
```

## Flutter application

```bash
cd flutter-app
flutter pub get
flutter test
```

The Flutter platform directories are generated only after the pinned Flutter
SDK is available. See the repository root README for the Windows and Android
setup sequence.

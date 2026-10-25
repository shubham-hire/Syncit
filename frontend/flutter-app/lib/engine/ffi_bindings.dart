/// Engine binding dispatcher.
///
/// On native platforms this resolves to the real dart:ffi implementation
/// (`ffi_native.dart`); on web it resolves to the stub (`ffi_stub.dart`),
/// whose `load()` returns null so the app falls back to mock mode. The
/// [FfiBindings] API is identical in both targets.
library;

export 'ffi_native.dart' if (dart.library.js_interop) 'ffi_stub.dart';

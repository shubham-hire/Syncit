/// Web fallback for the engine bindings: no dart:ffi on the web platform, so
/// `FfiBindings.load()` returns null and the app runs its mock session flows.
///
/// The instance methods mirror the native API surface so this target type
/// checks; they are unreachable in practice because the controller only uses
/// them after a successful `load()`.
library;

/// Stub of the native binding surface; every load returns null on web.
class FfiBindings {
  FfiBindings._();

  static FfiBindings? load() => null;

  static const String _unavailable =
      '{"ok":false,"error":"engine bindings are unavailable on web"}';

  String hostStart(String requestJson) => _unavailable;

  String hostCommand(String requestJson) => _unavailable;

  String hostStop(String requestJson) => _unavailable;

  String listenerStart(String requestJson) => _unavailable;

  String listenerPump(String requestJson) => _unavailable;

  String listenerLeave(String requestJson) => _unavailable;
}

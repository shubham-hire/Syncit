/// dart:ffi bindings for the AudioSync Rust engine (`audiosync_ffi`).
///
/// The library is built with
/// `cargo build -p audiosync-ffi --release` and loaded from the process
/// directory (`audiosync_ffi.dll` on Windows, `libaudiosync_ffi.dylib` on
/// macOS, `libaudiosync_ffi.so` on Android/Linux). When the library is
/// absent — widget tests, design-time previews — [FfiBindings.load] returns
/// null and the app falls back to the mock session state.
library;

import 'dart:ffi';
import 'dart:io';

import 'package:ffi/ffi.dart';

typedef _HostStartC = Pointer<Utf8> Function(Pointer<Utf8>);
typedef _HostStartDart = Pointer<Utf8> Function(Pointer<Utf8>);
typedef _CommandC = Pointer<Utf8> Function(Pointer<Utf8>);
typedef _CommandDart = Pointer<Utf8> Function(Pointer<Utf8>);
typedef _ListenerStartC = Pointer<Utf8> Function(Pointer<Utf8>);
typedef _ListenerStartDart = Pointer<Utf8> Function(Pointer<Utf8>);
typedef _PumpC = Pointer<Utf8> Function(Pointer<Utf8>);
typedef _PumpDart = Pointer<Utf8> Function(Pointer<Utf8>);
typedef _FreeStringC = Void Function(Pointer<Utf8>);
typedef _FreeStringDart = void Function(Pointer<Utf8>);

/// A loaded audiosync_ffi dynamic library.
class FfiBindings {
  FfiBindings._(this._library);

  final DynamicLibrary _library;

  /// Attempts to load the engine library; returns null when unavailable.
  ///
  /// Widget tests always run in mock mode: the Flutter test runner sets
  /// `FLUTTER_TEST`, and tests must be deterministic even on a machine
  /// where the engine library is built.
  static FfiBindings? load() {
    if (Platform.environment.containsKey('FLUTTER_TEST')) return null;
    final names = switch (Platform.operatingSystem) {
      'windows' => 'audiosync_ffi.dll',
      'macos' => 'libaudiosync_ffi.dylib',
      _ => 'libaudiosync_ffi.so',
    };
    for (final candidate in _searchPaths(names)) {
      try {
        return FfiBindings._(DynamicLibrary.open(candidate));
      } catch (_) {
        // Try the next candidate.
      }
    }
    return null;
  }

  /// Load candidates: the bare name (process directory / system search
  /// path, used by deployed apps), then the workspace output directories
  /// (`target/release/` and the crate-local fallback) found by walking up
  /// from the current directory and from the script location. Walking up
  /// covers `flutter run` from any depth and `dart run`, whose working
  /// directory is not the project directory.
  static List<String> _searchPaths(String file) {
    final paths = <String>[file];
    void walkFrom(String start) {
      var dir = File(start).parent.path;
      for (var depth = 0; depth < 8; depth++) {
        paths.add('$dir/target/release/$file');
        paths.add('$dir/backend/native/ffi/target/release/$file');
        final parent = Directory(dir).parent.path;
        if (parent == dir) break;
        dir = parent;
      }
    }

    walkFrom(Directory.current.path);
    walkFrom(Platform.script.toFilePath());
    return paths;
  }

  String _call(Pointer<Utf8> Function(Pointer<Utf8>) function, String request) {
    final argument = request.toNativeUtf8();
    final result = function(argument);
    final text = result.cast<Utf8>().toDartString();
    malloc.free(argument);
    final free = _library.lookupFunction<_FreeStringC, _FreeStringDart>(
      'audiosync_ffi_free_string',
    );
    free(result);
    return text;
  }

  /// Starts a host session. See the Rust docs for the request shape.
  String hostStart(String requestJson) => _call(
    _library.lookupFunction<_HostStartC, _HostStartDart>(
      'audiosync_ffi_host_start',
    ),
    requestJson,
  );

  /// Sends a host command and returns status plus retained events.
  String hostCommand(String requestJson) => _call(
    _library.lookupFunction<_CommandC, _CommandDart>(
      'audiosync_ffi_host_command',
    ),
    requestJson,
  );

  /// Stops a host loop and releases its native handle slot.
  String hostStop(String requestJson) => _call(
    _library.lookupFunction<_CommandC, _CommandDart>('audiosync_ffi_host_stop'),
    requestJson,
  );

  /// Joins a host session as a listener.
  String listenerStart(String requestJson) => _call(
    _library.lookupFunction<_ListenerStartC, _ListenerStartDart>(
      'audiosync_ffi_listener_start',
    ),
    requestJson,
  );

  /// Pumps the listener once and returns playback status plus events.
  String listenerPump(String requestJson) => _call(
    _library.lookupFunction<_PumpC, _PumpDart>('audiosync_ffi_listener_pump'),
    requestJson,
  );

  /// Leaves a listener session.
  String listenerLeave(String requestJson) => _call(
    _library.lookupFunction<_CommandC, _CommandDart>(
      'audiosync_ffi_listener_leave',
    ),
    requestJson,
  );
}

/// High-level controller over the AudioSync Rust engine.
///
/// Wraps [FfiBindings] in a small state-machine the Flutter pages can drive:
/// start a host (or join as a listener), poll status on a timer, issue
/// approve/remove/end commands, and expose parsed metrics. When the native
/// library is unavailable the controller reports [EngineAvailability.mock]
/// and the pages keep their existing mock behavior — widget tests run
/// entirely in this mode.
library;

import 'dart:async';
import 'dart:convert';

import 'ffi_bindings.dart';

/// Whether the real Rust engine is driving the session.
enum EngineAvailability { engine, mock }

/// Parsed listener playback status from one engine pump.
class ListenerStatus {
  ListenerStatus({
    required this.state,
    required this.active,
    required this.rendered,
    required this.concealed,
    required this.buffered,
    required this.roundTripMicros,
    required this.events,
  });

  factory ListenerStatus.fromMap(Map<String, dynamic> map) => ListenerStatus(
        state: map['state'] as String? ?? 'Disconnected',
        active: map['active'] == true,
        rendered: (map['rendered'] as num?)?.toInt() ?? 0,
        concealed: (map['concealed'] as num?)?.toInt() ?? 0,
        buffered: (map['buffered'] as num?)?.toInt() ?? 0,
        roundTripMicros: (map['rtt_us'] as num?)?.toInt() ?? -1,
        events: (map['events'] as List?)?.cast<String>() ?? const <String>[],
      );

  final String state;
  final bool active;
  final int rendered;
  final int concealed;
  final int buffered;
  final int roundTripMicros;
  final List<String> events;
}

/// Host session information echoed at start.
class HostSessionInfo {
  HostSessionInfo({
    required this.handle,
    required this.bindAddress,
    required this.sessionHex,
    required this.tokenHex,
  });

  factory HostSessionInfo.fromMap(Map<String, dynamic> map) => HostSessionInfo(
        handle: (map['handle'] as num?)?.toInt() ?? -1,
        bindAddress: map['bind'] as String? ?? '',
        sessionHex: map['session'] as String? ?? '',
        tokenHex: map['token'] as String? ?? '',
      );

  final int handle;
  final String bindAddress;
  final String sessionHex;
  final String tokenHex;
}

/// Owns one host or listener engine session.
class EngineController {
  EngineController._(this._bindings);

  final FfiBindings? _bindings;
  Timer? _poll;
  int _hostHandle = -1;
  int _listenerHandle = -1;

  /// Loads the engine when present; returns a mock-mode controller otherwise.
  static EngineController create() => EngineController._(FfiBindings.load());

  EngineAvailability get availability =>
      _bindings == null ? EngineAvailability.mock : EngineAvailability.engine;

  bool get hasHost => _hostHandle >= 0;
  bool get hasListener => _listenerHandle >= 0;

  Map<String, dynamic> _decode(String text) {
    try {
      return (jsonDecode(text) as Map).cast<String, dynamic>();
    } catch (_) {
      return <String, dynamic>{'ok': false, 'error': 'unparseable engine response'};
    }
  }

  /// Starts a host session. Returns null in mock mode.
  HostSessionInfo? startHost({
    String bind = '0.0.0.0:41000',
    int maxListeners = 5,
    bool synthetic = true,
    bool autoApprove = true,
  }) {
    final bindings = _bindings;
    if (bindings == null) return null;
    final response = _decode(
      bindings.hostStart(
        jsonEncode({
          'bind': bind,
          'max_listeners': maxListeners,
          'synthetic': synthetic,
          'auto_approve': autoApprove,
        }),
      ),
    );
    if (response['ok'] != true) {
      throw StateError(response['error'] as String? ?? 'host start failed');
    }
    final info = HostSessionInfo.fromMap(response);
    _hostHandle = info.handle;
    return info;
  }

  /// Sends approve/remove/end to the running host and returns raw events.
  List<String> hostCommand(String command, {String? listenerHex}) {
    final bindings = _bindings;
    if (bindings == null || _hostHandle < 0) return const [];
    final response = _decode(
      bindings.hostCommand(
        jsonEncode({
          'handle': _hostHandle,
          'command': command,
          if (listenerHex != null) 'listener': listenerHex,
        }),
      ),
    );
    return (response['events'] as List?)?.cast<String>() ?? const <String>[];
  }

  /// Joins a host as a listener. Returns false in mock mode.
  bool joinListener({
    required String host,
    required String sessionHex,
    required String tokenHex,
    int delayMs = 0,
    int bufferMs = 100,
  }) {
    final bindings = _bindings;
    if (bindings == null) return false;
    final response = _decode(
      bindings.listenerStart(
        jsonEncode({
          'host': host,
          'session': sessionHex,
          'token': tokenHex,
          'delay_ms': delayMs,
          'buffer_ms': bufferMs,
        }),
      ),
    );
    if (response['ok'] != true) {
      throw StateError(response['error'] as String? ?? 'join failed');
    }
    _listenerHandle = (response['handle'] as num).toInt();
    return true;
  }

  /// Pumps the listener once. Returns null in mock mode.
  ListenerStatus? pumpListener({int? delayMs}) {
    final bindings = _bindings;
    if (bindings == null || _listenerHandle < 0) return null;
    final response = _decode(
      bindings.listenerPump(
        jsonEncode({
          'handle': _listenerHandle,
          if (delayMs != null) 'delay_ms': delayMs,
        }),
      ),
    );
    return ListenerStatus.fromMap(response);
  }

  /// Polls the host for retained events (joins, approvals, timeouts) without
  /// sending a command; drives the host page's listener list.
  void startHostPolling(
    void Function(List<String> events) onEvents, {
    Duration interval = const Duration(milliseconds: 250),
  }) {
    stopPolling();
    final bindings = _bindings;
    if (bindings == null) return;
    _poll = Timer.periodic(interval, (_) {
      if (_hostHandle < 0) return;
      final response = _decode(
        bindings.hostCommand('{"handle":$_hostHandle}'),
      );
      onEvents(
        (response['events'] as List?)?.cast<String>() ?? const <String>[],
      );
    });
  }

  /// Polls [onStatus] at [interval] until [stopPolling] is called.
  void startListenerPolling(
    void Function(ListenerStatus) onStatus, {
    Duration interval = const Duration(milliseconds: 100),
  }) {
    stopPolling();
    _poll = Timer.periodic(interval, (_) {
      final status = pumpListener();
      if (status != null) onStatus(status);
    });
  }

  void stopPolling() {
    _poll?.cancel();
    _poll = null;
  }

  /// Ends the host session and releases both handles.
  void dispose() {
    stopPolling();
    if (_bindings != null && _hostHandle >= 0) {
      _bindings.hostCommand('{"handle":$_hostHandle,"command":"end"}');
    }
    if (_bindings != null && _listenerHandle >= 0) {
      _bindings.listenerLeave('{"handle":$_listenerHandle}');
    }
    _hostHandle = -1;
    _listenerHandle = -1;
  }
}

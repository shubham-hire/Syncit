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

List<String> _stringList(Object? value) {
  if (value is! List) return const <String>[];
  return value.whereType<String>().toList(growable: false);
}

/// Parsed host command/polling status from one engine call.
class HostStatus {
  HostStatus({
    required this.running,
    required this.events,
    required this.errors,
  });

  factory HostStatus.fromMap(Map<String, dynamic> map) {
    final errors = <String>[..._stringList(map['errors'])];
    final requestError = map['error'];
    if (requestError is String && requestError.isNotEmpty) {
      errors.add(requestError);
    }
    return HostStatus(
      running: map['running'] == true,
      events: _stringList(map['events']),
      errors: errors,
    );
  }

  final bool running;
  final List<String> events;
  final List<String> errors;
}

/// Parsed listener playback status from one engine pump.
class ListenerStatus {
  ListenerStatus({
    required this.state,
    required this.active,
    required this.rendered,
    required this.concealed,
    required this.received,
    required this.inserted,
    required this.invalid,
    required this.wrongSession,
    required this.unauthenticated,
    required this.duplicate,
    required this.late,
    required this.capacityDrops,
    required this.sinkFailures,
    required this.watchdogTimeouts,
    required this.rejoinAttempts,
    required this.buffered,
    required this.roundTripMicros,
    required this.offsetMicros,
    required this.output,
    required this.events,
  });

  factory ListenerStatus.fromMap(Map<String, dynamic> map) => ListenerStatus(
    state: map['state'] as String? ?? 'Disconnected',
    active: map['active'] == true,
    rendered: (map['rendered'] as num?)?.toInt() ?? 0,
    concealed: (map['concealed'] as num?)?.toInt() ?? 0,
    received: (map['received'] as num?)?.toInt() ?? 0,
    inserted: (map['inserted'] as num?)?.toInt() ?? 0,
    invalid: (map['invalid'] as num?)?.toInt() ?? 0,
    wrongSession: (map['wrong_session'] as num?)?.toInt() ?? 0,
    unauthenticated: (map['unauthenticated'] as num?)?.toInt() ?? 0,
    duplicate: (map['duplicate'] as num?)?.toInt() ?? 0,
    late: (map['late'] as num?)?.toInt() ?? 0,
    capacityDrops: (map['capacity_drops'] as num?)?.toInt() ?? 0,
    sinkFailures: (map['sink_failures'] as num?)?.toInt() ?? 0,
    watchdogTimeouts: (map['watchdog_timeouts'] as num?)?.toInt() ?? 0,
    rejoinAttempts: (map['rejoin_attempts'] as num?)?.toInt() ?? 0,
    buffered: (map['buffered'] as num?)?.toInt() ?? 0,
    roundTripMicros: (map['rtt_us'] as num?)?.toInt() ?? -1,
    offsetMicros: (map['offset_us'] as num?)?.toInt() ?? 0,
    output: map['output'] as String? ?? 'unknown',
    events: _stringList(map['events']),
  );

  final String state;
  final bool active;
  final int rendered;
  final int concealed;
  final int received;
  final int inserted;
  final int invalid;
  final int wrongSession;
  final int unauthenticated;
  final int duplicate;
  final int late;
  final int capacityDrops;
  final int sinkFailures;
  final int watchdogTimeouts;
  final int rejoinAttempts;
  final int buffered;
  final int roundTripMicros;
  final int offsetMicros;
  final String output;
  final List<String> events;
}

/// Host session information echoed at start.
class HostSessionInfo {
  HostSessionInfo({
    required this.handle,
    required this.bindAddress,
    required this.sessionHex,
    required this.tokenHex,
    required this.source,
  });

  factory HostSessionInfo.fromMap(Map<String, dynamic> map) => HostSessionInfo(
    handle: (map['handle'] as num?)?.toInt() ?? -1,
    bindAddress: map['bind'] as String? ?? '',
    sessionHex: map['session'] as String? ?? '',
    tokenHex: map['token'] as String? ?? '',
    source: map['source'] as String? ?? 'unknown',
  );

  final int handle;
  final String bindAddress;
  final String sessionHex;
  final String tokenHex;
  final String source;
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
      return <String, dynamic>{
        'ok': false,
        'error': 'unparseable engine response',
      };
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

  /// Sends approve/remove/end to the running host and returns diagnostics.
  HostStatus hostCommand(String command, {String? listenerHex}) {
    final bindings = _bindings;
    if (bindings == null || _hostHandle < 0) {
      return HostStatus(running: false, events: const [], errors: const []);
    }
    final response = _decode(
      bindings.hostCommand(
        jsonEncode({
          'handle': _hostHandle,
          'command': command,
          if (listenerHex != null) 'listener': listenerHex,
        }),
      ),
    );
    return HostStatus.fromMap(response);
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
    void Function(HostStatus status) onStatus, {
    Duration interval = const Duration(milliseconds: 250),
  }) {
    stopPolling();
    final bindings = _bindings;
    if (bindings == null) return;
    _poll = Timer.periodic(interval, (_) {
      if (_hostHandle < 0) return;
      final response = _decode(bindings.hostCommand('{"handle":$_hostHandle}'));
      onStatus(HostStatus.fromMap(response));
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
      _bindings.hostStop('{"handle":$_hostHandle}');
    }
    if (_bindings != null && _listenerHandle >= 0) {
      _bindings.listenerLeave('{"handle":$_listenerHandle}');
    }
    _hostHandle = -1;
    _listenerHandle = -1;
  }
}

/// One event drained from host polling, parsed into a name and an optional
/// listener hex id.
class HostEvent {
  HostEvent({required this.name, this.listenerHex});

  final String name;

  /// 32-char lowercase hex listener id when the event carries one.
  final String? listenerHex;

  /// Parses one raw event string. The engine serializes each event as
  /// `{"event":"<Rust Debug of the SessionEvent>"}` and Rust renders ids as
  /// `ListenerId([byte, byte, ...])`, so the hex is rebuilt from the bytes.
  static HostEvent? parse(String raw) {
    var normalized = raw.trim();
    // Older FFI DLLs emitted an extra comma after the nested event object.
    // Accept it while deployed DLLs are being refreshed after the fix.
    if (normalized.endsWith(',')) {
      normalized = normalized.substring(0, normalized.length - 1).trimRight();
    }
    var text = normalized;
    try {
      final decoded = jsonDecode(normalized);
      if (decoded is Map && decoded['event'] is String) {
        text = decoded['event'] as String;
      }
    } catch (_) {
      // Not JSON; treat the raw text as the event payload.
    }
    final name = RegExp(r'^([A-Za-z]+)').firstMatch(text)?.group(1);
    if (name == null) return null;
    return HostEvent(name: name, listenerHex: _listenerHex(text));
  }

  static String? _listenerHex(String text) {
    final match = RegExp(
      r'ListenerId\(\[([0-9,\s]+)\]\)',
    ).firstMatch(text)?.group(1);
    if (match == null) return null;
    final bytes = match.split(',').map((part) => int.tryParse(part.trim()));
    if (bytes.any((byte) => byte == null || byte < 0 || byte > 255)) {
      return null;
    }
    final hex =
        bytes.map((byte) => byte!.toRadixString(16).padLeft(2, '0')).join();
    return hex.length == 32 ? hex : null;
  }
}

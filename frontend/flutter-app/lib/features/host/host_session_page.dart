import 'package:flutter/material.dart';

import '../../core/widgets/status_card.dart';
import '../../engine/engine_controller.dart';
import 'widgets/hotspot_instructions.dart';
import 'widgets/listener_list.dart';
import 'widgets/session_code_card.dart';

class HostSessionPage extends StatefulWidget {
  const HostSessionPage({super.key});

  @override
  State<HostSessionPage> createState() => _HostSessionPageState();
}

class _HostSessionPageState extends State<HostSessionPage> {
  final EngineController _engine = EngineController.create();

  bool _sessionStarted = false;
  bool get _engineMode => _engine.availability == EngineAvailability.engine;

  // Mock-mode state (widget tests drive exactly this).
  final List<String> _listeners = <String>[];
  final Set<String> _approvedListeners = <String>{};

  // Engine-mode state.
  HostSessionInfo? _sessionInfo;
  final Map<String, String> _engineListenerNames = <String, String>{};
  final Set<String> _engineApproved = <String>{};

  @override
  void dispose() {
    _engine.dispose();
    super.dispose();
  }

  void _startSession() {
    if (_engineMode) {
      try {
        final info = _engine.startHost(synthetic: false, autoApprove: false);
        setState(() {
          _sessionStarted = true;
          _sessionInfo = info;
        });
        _engine.startHostPolling(_onHostEvents);
      } on StateError catch (error) {
        _showSnack('Engine could not start the host: ${error.message}');
      }
      return;
    }
    setState(() => _sessionStarted = true);
  }

  void _onHostEvents(List<String> events) {
    var changed = false;
    for (final raw in events) {
      final event = HostEvent.parse(raw);
      if (event == null || event.listenerHex == null) continue;
      final hex = event.listenerHex!;
      switch (event.name) {
        case 'ListenerJoined':
        case 'ListenerReconnected':
          if (!_engineListenerNames.containsKey(hex)) {
            _engineListenerNames[hex] =
                'Listener ${_engineListenerNames.length + 1}';
            changed = true;
          }
        case 'ListenerApproved':
          if (_engineApproved.add(hex)) changed = true;
        case 'ListenerRemoved':
        case 'ListenerTimedOut':
          _engineListenerNames.remove(hex);
          _engineApproved.remove(hex);
          changed = true;
      }
    }
    if (changed) setState(() {});
  }

  void _addMockListener() {
    setState(() {
      _listeners.add('Listener ${_listeners.length + 1}');
    });
  }

  void _approveListener(String listener) {
    if (_engineMode) {
      final hex =
          _engineListenerNames.entries
              .where((entry) => entry.value == listener)
              .map((entry) => entry.key)
              .firstOrNull;
      if (hex != null) _engine.hostCommand('approve', listenerHex: hex);
      return;
    }
    setState(() => _approvedListeners.add(listener));
  }

  void _removeListener(String listener) {
    if (_engineMode) {
      final hex =
          _engineListenerNames.entries
              .where((entry) => entry.value == listener)
              .map((entry) => entry.key)
              .firstOrNull;
      if (hex != null) {
        _engine.hostCommand('remove', listenerHex: hex);
        setState(() {
          _engineListenerNames.remove(hex);
          _engineApproved.remove(hex);
        });
      }
      return;
    }
    setState(() {
      _listeners.remove(listener);
      _approvedListeners.remove(listener);
    });
  }

  void _showSnack(String message) {
    if (!mounted) return;
    ScaffoldMessenger.of(
      context,
    ).showSnackBar(SnackBar(content: Text(message)));
  }

  List<String> get _displayListeners =>
      _engineMode ? _engineListenerNames.values.toList() : _listeners;
  Set<String> get _displayApproved =>
      _engineMode ? _engineApproved : _approvedListeners;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Host session')),
      body: Center(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 720),
          child: ListView(
            padding: const EdgeInsets.all(24),
            children: [
              StatusCard(
                icon: _sessionStarted ? Icons.play_circle : Icons.pause_circle,
                title:
                    _sessionStarted
                        ? 'Session is ready'
                        : 'Session not started',
                message:
                    _sessionStarted
                        ? 'Listeners can join using the session information below.'
                        : 'Start a local session after enabling the host hotspot.',
                action:
                    _sessionStarted
                        ? null
                        : FilledButton(
                          onPressed: _startSession,
                          child: const Text('Start session'),
                        ),
              ),
              const SizedBox(height: 16),
              const HotspotInstructions(),
              if (_sessionStarted) ...[
                const SizedBox(height: 16),
                SessionCodeCard(
                  sessionHex: _sessionInfo?.sessionHex,
                  tokenHex: _sessionInfo?.tokenHex,
                ),
                const SizedBox(height: 16),
                ListenerList(
                  listeners: _displayListeners,
                  approvedListeners: _displayApproved,
                  transportLabel:
                      _engineMode ? 'Rust engine' : 'mock transport',
                  onSimulateListener: _engineMode ? null : _addMockListener,
                  onApprove: _approveListener,
                  onRemove: _removeListener,
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}

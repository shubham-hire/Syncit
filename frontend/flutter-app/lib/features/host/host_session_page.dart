import 'package:flutter/material.dart';

import '../../core/theme/liquid_glass.dart';
import '../../core/widgets/status_card.dart';
import '../../engine/engine_controller.dart';
import 'widgets/hotspot_instructions.dart';
import 'widgets/listener_list.dart';
import 'widgets/session_code_card.dart';

enum _ApprovalAction { approve, remove }

class HostSessionPage extends StatefulWidget {
  const HostSessionPage({super.key});

  @override
  State<HostSessionPage> createState() => _HostSessionPageState();
}

class _HostSessionPageState extends State<HostSessionPage> {
  final EngineController _engine = EngineController.create();

  bool _sessionStarted = false;
  bool _syntheticSource = false;
  bool get _engineMode => _engine.availability == EngineAvailability.engine;

  // Mock-mode state (widget tests drive exactly this).
  final List<String> _listeners = <String>[];
  final Set<String> _approvedListeners = <String>{};

  // Engine-mode state.
  HostSessionInfo? _sessionInfo;
  final Map<String, String> _engineListenerNames = <String, String>{};
  final Set<String> _engineApproved = <String>{};
  final List<String> _approvalQueue = <String>[];
  final List<String> _hostEvents = <String>[];
  final List<String> _hostErrors = <String>[];
  HostStatus? _hostStatus;
  bool _approvalDialogVisible = false;

  @override
  void dispose() {
    _engine.dispose();
    super.dispose();
  }

  void _startSession() {
    if (_engineMode) {
      try {
        final info = _engine.startHost(
          synthetic: _syntheticSource,
          autoApprove: false,
        );
        setState(() {
          _sessionStarted = true;
          _sessionInfo = info;
        });
        _engine.startHostPolling(_onHostStatus);
      } on StateError catch (error) {
        _showSnack('Engine could not start the host: ${error.message}');
      }
      return;
    }
    setState(() => _sessionStarted = true);
  }

  void _onHostStatus(HostStatus status) {
    var changed = false;
    for (final raw in status.events) {
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
          if (!_engineApproved.contains(hex)) _enqueueApproval(hex);
        case 'ListenerApproved':
          _approvalQueue.remove(hex);
          if (_engineApproved.add(hex)) changed = true;
        case 'ListenerRemoved':
        case 'ListenerTimedOut':
          _approvalQueue.remove(hex);
          _engineListenerNames.remove(hex);
          _engineApproved.remove(hex);
          changed = true;
      }
    }
    if (!mounted) return;
    setState(() {
      _hostStatus = status;
      if (status.events.isNotEmpty) _retain(_hostEvents, status.events);
      if (status.errors.isNotEmpty) _retain(_hostErrors, status.errors);
    });
    if (changed || status.events.isNotEmpty) _showNextApprovalDialog();
  }

  void _retain(List<String> target, Iterable<String> values) {
    target.addAll(values);
    if (target.length > 8) {
      target.removeRange(0, target.length - 8);
    }
  }

  String _hostEventLabel(String raw) {
    final event = HostEvent.parse(raw);
    return event?.name ?? raw;
  }

  String get _hostDiagnostics {
    final status = _hostStatus;
    if (status == null) return 'Waiting for the first host poll…';
    final lines = <String>[
      'running ${status.running}',
      'source ${_sessionInfo?.source ?? 'unknown'}',
    ];
    if (_hostEvents.isNotEmpty) {
      lines.add('events ${_hostEvents.map(_hostEventLabel).join(', ')}');
    }
    if (_hostErrors.isNotEmpty) {
      lines.add('errors\n${_hostErrors.join('\n')}');
    } else {
      lines.add('errors none');
    }
    return lines.join('\n');
  }

  void _handleHostCommand(HostStatus status) {
    _onHostStatus(status);
  }

  void _enqueueApproval(String listenerHex) {
    if (!_approvalQueue.contains(listenerHex)) {
      _approvalQueue.add(listenerHex);
    }
  }

  void _reviewPendingApprovals() {
    for (final entry in _engineListenerNames.entries) {
      if (!_engineApproved.contains(entry.key)) _enqueueApproval(entry.key);
    }
    _showNextApprovalDialog();
  }

  void _showNextApprovalDialog() {
    if (!mounted || !_engineMode || _approvalDialogVisible) return;
    while (_approvalQueue.isNotEmpty) {
      final listenerHex = _approvalQueue.removeAt(0);
      final listener = _engineListenerNames[listenerHex];
      if (listener == null || _engineApproved.contains(listenerHex)) continue;

      _approvalDialogVisible = true;
      WidgetsBinding.instance.addPostFrameCallback((_) async {
        if (!mounted) {
          _approvalDialogVisible = false;
          return;
        }
        final action = await showDialog<_ApprovalAction>(
          context: context,
          builder:
              (dialogContext) => AlertDialog(
                title: const Text('Listener waiting for approval'),
                content: Text(
                  '$listener is requesting to join this AudioSync session.',
                ),
                actions: [
                  TextButton(
                    onPressed:
                        () => Navigator.of(
                          dialogContext,
                        ).pop(_ApprovalAction.remove),
                    child: const Text('Reject'),
                  ),
                  FilledButton(
                    onPressed:
                        () => Navigator.of(
                          dialogContext,
                        ).pop(_ApprovalAction.approve),
                    child: const Text('Approve'),
                  ),
                ],
              ),
        );
        if (!mounted) return;
        _approvalDialogVisible = false;
        if (action == _ApprovalAction.approve) {
          _approveListener(listener);
        } else if (action == _ApprovalAction.remove) {
          _removeListener(listener);
        }
        _showNextApprovalDialog();
      });
      return;
    }
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
      if (hex != null) {
        _handleHostCommand(_engine.hostCommand('approve', listenerHex: hex));
      }
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
        _approvalQueue.remove(hex);
        _handleHostCommand(_engine.hostCommand('remove', listenerHex: hex));
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
  int get _pendingEngineListeners =>
      _engineListenerNames.keys
          .where((hex) => !_engineApproved.contains(hex))
          .length;

  @override
  Widget build(BuildContext context) {
    return GlassScaffold(
      title: 'Host session',
      largeTitle: 'Host session',
      subtitle: 'Share this device\'s audio over a local Wi-Fi hotspot.',
      children: [
        StatusCard(
          icon: _sessionStarted ? Icons.play_circle : Icons.pause_circle,
          title: _sessionStarted ? 'Session is ready' : 'Session not started',
          message:
              _sessionStarted
                  ? 'Listeners can join using the session information below.'
                  : 'Start a local session after enabling the host hotspot.',
          action:
              _sessionStarted
                  ? null
                  : AppPillButton(
                    label: 'Start session',
                    onPressed: _startSession,
                  ),
        ),
        const SizedBox(height: 16),
        const HotspotInstructions(),
        if (_engineMode && !_sessionStarted) ...[
          const SizedBox(height: 16),
          Card(
            child: SwitchListTile(
              title: const Text('Use synthetic test tone'),
              subtitle: const Text(
                'Diagnostic mode bypasses WASAPI and streams a generated tone.',
              ),
              value: _syntheticSource,
              onChanged: (value) {
                setState(() => _syntheticSource = value);
              },
            ),
          ),
        ],
        if (_sessionStarted) ...[
          const SizedBox(height: 16),
          SessionCodeCard(
            sessionHex: _sessionInfo?.sessionHex,
            tokenHex: _sessionInfo?.tokenHex,
          ),
          if (_engineMode) ...[
            const SizedBox(height: 16),
            if (_sessionInfo?.source == 'WASAPI loopback')
              const StatusCard(
                icon: Icons.warning_amber,
                title: 'WASAPI same-device caution',
                message:
                    'WASAPI captures the host Windows default output. '
                    'If a listener runs on this same PC and also plays to '
                    'the default output, you will hear the original audio '
                    'and the delayed relayed copy together. Use a separate '
                    'listener device or output device for a clean test.',
              ),
            if (_sessionInfo?.source == 'WASAPI loopback')
              const SizedBox(height: 16),
            StatusCard(
              icon:
                  _hostErrors.isEmpty
                      ? Icons.monitor_heart
                      : Icons.error_outline,
              title: 'Host diagnostics',
              message: _hostDiagnostics,
            ),
          ],
          if (_engineMode && _pendingEngineListeners > 0) ...[
            const SizedBox(height: 16),
            StatusCard(
              icon: Icons.pending_actions,
              title:
                  '$_pendingEngineListeners listener${_pendingEngineListeners == 1 ? '' : 's'} waiting for approval',
              message: 'Review the request before audio streaming begins.',
              action: AppPillButton(
                label: 'Review request',
                icon: Icons.rate_review,
                onPressed: _reviewPendingApprovals,
              ),
            ),
          ],
          const SizedBox(height: 16),
          ListenerList(
            listeners: _displayListeners,
            approvedListeners: _displayApproved,
            transportLabel: _engineMode ? 'Rust engine' : 'mock transport',
            onSimulateListener: _engineMode ? null : _addMockListener,
            onApprove: _approveListener,
            onRemove: _removeListener,
          ),
        ],
      ],
    );
  }
}

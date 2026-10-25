import 'package:flutter/material.dart';

import '../../core/theme/liquid_glass.dart';
import '../../core/widgets/status_card.dart';
import '../../engine/engine_controller.dart';
import '../session/session_constants.dart';
import 'widgets/join_form.dart';
import 'widgets/playback_controls.dart';

class ListenerJoinPage extends StatefulWidget {
  const ListenerJoinPage({super.key});

  @override
  State<ListenerJoinPage> createState() => _ListenerJoinPageState();
}

class _ListenerJoinPageState extends State<ListenerJoinPage> {
  final EngineController _engine = EngineController.create();

  final TextEditingController _codeController = TextEditingController();
  final TextEditingController _hostController = TextEditingController();
  final TextEditingController _sessionController = TextEditingController();
  final TextEditingController _tokenController = TextEditingController();

  bool _joined = false;
  double _delay = 0;
  double _volume = 0.8;
  bool _muted = false;
  ListenerStatus? _status;
  final List<String> _listenerEvents = <String>[];

  bool get _engineMode => _engine.availability == EngineAvailability.engine;

  @override
  void dispose() {
    _engine.dispose();
    _codeController.dispose();
    _hostController.dispose();
    _sessionController.dispose();
    _tokenController.dispose();
    super.dispose();
  }

  void _join() {
    if (_engineMode) {
      _joinWithEngine();
      return;
    }
    final code = _codeController.text.trim().toUpperCase();
    if (code.isEmpty) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Enter the session code first.')),
      );
      return;
    }
    if (code != mockSessionCode) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('That mock session code is not valid.')),
      );
      return;
    }
    setState(() => _joined = true);
  }

  void _joinWithEngine() {
    final host = _hostController.text.trim();
    final sessionHex = _sessionController.text.trim().toLowerCase();
    final tokenHex = _tokenController.text.trim().toLowerCase();
    if (host.isEmpty || sessionHex.isEmpty || tokenHex.isEmpty) {
      _showSnack('Enter the host address, session and token.');
      return;
    }
    if (!isValidHex32(sessionHex)) {
      _showSnack('The session must be 32 hex characters.');
      return;
    }
    if (!isValidHex32(tokenHex)) {
      _showSnack('The token must be 32 hex characters.');
      return;
    }
    try {
      _engine.joinListener(
        host: host,
        sessionHex: sessionHex,
        tokenHex: tokenHex,
      );
    } on StateError catch (error) {
      _showSnack('Engine could not join: ${error.message}');
      return;
    }
    setState(() => _joined = true);
    _engine.startListenerPolling(_onListenerStatus);
  }

  void _onListenerStatus(ListenerStatus status) {
    if (!mounted) return;
    setState(() {
      _status = status;
      if (status.events.isNotEmpty) {
        _listenerEvents.addAll(status.events);
        if (_listenerEvents.length > 8) {
          _listenerEvents.removeRange(0, _listenerEvents.length - 8);
        }
      }
    });
  }

  void _showSnack(String message) {
    if (!mounted) return;
    ScaffoldMessenger.of(
      context,
    ).showSnackBar(SnackBar(content: Text(message)));
  }

  String get _engineStatusLine {
    final status = _status;
    if (status == null) return 'Waiting for the first engine pump…';
    final state =
        status.state == 'PendingApproval'
            ? 'Waiting for host approval'
            : status.state;
    return '$state · rendered ${status.rendered} · '
        'concealed ${status.concealed} · buffer ${status.buffered} · '
        'RTT ${status.roundTripMicros < 0 ? '-' : '${status.roundTripMicros ~/ 1000} ms'} · '
        'output ${status.output}';
  }

  String get _listenerDiagnostics {
    final status = _status;
    if (status == null) return 'Waiting for the first engine pump…';
    final lines = <String>[
      'received ${status.received} · inserted ${status.inserted} · '
          'buffer ${status.buffered}',
      'invalid ${status.invalid} · wrong session ${status.wrongSession} · '
          'unauthenticated ${status.unauthenticated}',
      'duplicate ${status.duplicate} · late ${status.late} · '
          'capacity drops ${status.capacityDrops}',
      'clock offset ${status.offsetMicros ~/ 1000} ms · '
          'sink failures ${status.sinkFailures} · '
          'watchdog timeouts ${status.watchdogTimeouts} · '
          'rejoins ${status.rejoinAttempts}',
    ];
    if (_listenerEvents.isNotEmpty) {
      lines.add('events\n${_listenerEvents.join('\n')}');
    } else {
      lines.add('events none');
    }
    return lines.join('\n');
  }

  bool get _isLoopbackHost {
    final host = _hostController.text.trim().toLowerCase();
    return host == 'localhost' ||
        host.startsWith('localhost:') ||
        host.startsWith('127.') ||
        host.startsWith('[::1]');
  }

  @override
  Widget build(BuildContext context) {
    return GlassScaffold(
      title: 'Listen to a session',
      children: [
              if (!_joined)
                JoinForm(
                  codeController: _codeController,
                  onJoin: _join,
                  engineMode: _engineMode,
                  hostController: _hostController,
                  sessionController: _sessionController,
                  tokenController: _tokenController,
                ),
              if (_joined) ...[
                if (_engineMode) ...[
                  StatusCard(
                    icon:
                        (_status?.active ?? false)
                            ? Icons.surround_sound
                            : _status?.state == 'PendingApproval'
                            ? Icons.pending_actions
                            : Icons.sync,
                    title:
                        (_status?.active ?? false)
                            ? 'Session connected'
                            : _status?.state == 'PendingApproval'
                            ? 'Waiting for host approval'
                            : 'Connecting to session',
                    message: _engineStatusLine,
                  ),
                  const SizedBox(height: 8),
                  Text(
                    'Decoded audio is sent to the Windows default output. '
                    'Volume and mute controls remain local UI controls for now.',
                    style: Theme.of(context).textTheme.bodySmall,
                  ),
                  if (_isLoopbackHost &&
                      _status?.output == 'Windows default output') ...[
                    const SizedBox(height: 16),
                    const StatusCard(
                      icon: Icons.warning_amber,
                      title: 'Same-PC loopback warning',
                      message:
                          'The host captures the Windows default output and '
                          'this listener also plays to that output. On the same '
                          'PC, this produces the original audio plus a delayed '
                          'relayed copy. Use a separate listener device or choose '
                          'a different output device.',
                    ),
                  ],
                  const SizedBox(height: 16),
                  StatusCard(
                    icon: Icons.analytics_outlined,
                    title: 'Listener diagnostics',
                    message: _listenerDiagnostics,
                  ),
                ] else ...[
                  const StatusCard(
                    icon: Icons.check_circle,
                    title: 'Mock session connected',
                    message:
                        'Playback controls are ready for the real listener engine.',
                  ),
                ],
                const SizedBox(height: 16),
                PlaybackControls(
                  delay: _delay,
                  volume: _volume,
                  muted: _muted,
                  onDelayChanged: (value) {
                    setState(() => _delay = value);
                    if (_engineMode) {
                      _engine.pumpListener(delayMs: value.round());
                    }
                  },
                  onVolumeChanged: (value) => setState(() => _volume = value),
                  onMuteChanged: (value) => setState(() => _muted = value),
                ),
              ],
      ],
    );
  }
}

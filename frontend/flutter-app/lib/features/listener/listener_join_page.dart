import 'package:flutter/material.dart';

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
    _engine.startListenerPolling((status) {
      if (mounted) setState(() => _status = status);
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
    return '${status.state} · rendered ${status.rendered} · '
        'concealed ${status.concealed} · buffer ${status.buffered} · '
        'RTT ${status.roundTripMicros < 0 ? '-' : '${status.roundTripMicros ~/ 1000} ms'}';
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Listen to a session')),
      body: Center(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 720),
          child: ListView(
            padding: const EdgeInsets.all(24),
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
                            : Icons.sync,
                    title: 'Session connected',
                    message: _engineStatusLine,
                  ),
                  const SizedBox(height: 8),
                  Text(
                    'Volume is applied on-device at playback (Milestone 6); '
                    'the engine currently renders into a null sink.',
                    style: Theme.of(context).textTheme.bodySmall,
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
          ),
        ),
      ),
    );
  }
}

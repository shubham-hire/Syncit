import 'package:flutter/material.dart';

import '../../core/widgets/status_card.dart';
import '../session/session_constants.dart';
import 'widgets/join_form.dart';
import 'widgets/playback_controls.dart';

class ListenerJoinPage extends StatefulWidget {
  const ListenerJoinPage({super.key});

  @override
  State<ListenerJoinPage> createState() => _ListenerJoinPageState();
}

class _ListenerJoinPageState extends State<ListenerJoinPage> {
  final TextEditingController _codeController = TextEditingController();
  bool _joined = false;
  double _delay = 0;
  double _volume = 0.8;
  bool _muted = false;

  @override
  void dispose() {
    _codeController.dispose();
    super.dispose();
  }

  void _join() {
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
                JoinForm(codeController: _codeController, onJoin: _join),
              if (_joined) ...[
                const StatusCard(
                  icon: Icons.check_circle,
                  title: 'Mock session connected',
                  message:
                      'Playback controls are ready for the real listener engine.',
                ),
                const SizedBox(height: 16),
                PlaybackControls(
                  delay: _delay,
                  volume: _volume,
                  muted: _muted,
                  onDelayChanged: (value) => setState(() => _delay = value),
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

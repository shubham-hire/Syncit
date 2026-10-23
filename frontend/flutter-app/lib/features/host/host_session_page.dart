import 'package:flutter/material.dart';

import '../../core/widgets/status_card.dart';
import 'widgets/hotspot_instructions.dart';
import 'widgets/listener_list.dart';
import 'widgets/session_code_card.dart';

class HostSessionPage extends StatefulWidget {
  const HostSessionPage({super.key});

  @override
  State<HostSessionPage> createState() => _HostSessionPageState();
}

class _HostSessionPageState extends State<HostSessionPage> {
  bool _sessionStarted = false;
  final List<String> _listeners = <String>[];
  final Set<String> _approvedListeners = <String>{};

  void _startSession() {
    setState(() => _sessionStarted = true);
  }

  void _addMockListener() {
    setState(() {
      _listeners.add('Listener ${_listeners.length + 1}');
    });
  }

  void _approveListener(String listener) {
    setState(() => _approvedListeners.add(listener));
  }

  void _removeListener(String listener) {
    setState(() {
      _listeners.remove(listener);
      _approvedListeners.remove(listener);
    });
  }

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
                const SessionCodeCard(),
                const SizedBox(height: 16),
                ListenerList(
                  listeners: _listeners,
                  approvedListeners: _approvedListeners,
                  onSimulateListener: _addMockListener,
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

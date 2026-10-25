import 'package:flutter/material.dart';

/// Listener join form. Mock mode shows the single short-code field the
/// widget tests drive; engine mode collects the host address plus the
/// session/token hex printed by the host.
class JoinForm extends StatelessWidget {
  const JoinForm({
    required this.codeController,
    required this.onJoin,
    this.engineMode = false,
    this.hostController,
    this.sessionController,
    this.tokenController,
    super.key,
  });

  final TextEditingController codeController;
  final VoidCallback onJoin;
  final bool engineMode;
  final TextEditingController? hostController;
  final TextEditingController? sessionController;
  final TextEditingController? tokenController;

  @override
  Widget build(BuildContext context) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(20),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              'Join a host',
              style: Theme.of(
                context,
              ).textTheme.titleLarge?.copyWith(fontWeight: FontWeight.w700),
            ),
            const SizedBox(height: 12),
            const Text(
              'Connect to the host hotspot, then scan its QR code or enter the short session code below.',
            ),
            const SizedBox(height: 20),
            if (engineMode) ...[
              TextField(
                controller: hostController,
                keyboardType: TextInputType.url,
                decoration: const InputDecoration(
                  border: OutlineInputBorder(),
                  labelText: 'Host address',
                  hintText: '192.168.137.1:41000',
                ),
              ),
              const SizedBox(height: 16),
              TextField(
                controller: sessionController,
                decoration: const InputDecoration(
                  border: OutlineInputBorder(),
                  labelText: 'Session (hex)',
                ),
              ),
              const SizedBox(height: 16),
              TextField(
                controller: tokenController,
                decoration: const InputDecoration(
                  border: OutlineInputBorder(),
                  labelText: 'Token (hex)',
                ),
              ),
            ] else
              TextField(
                controller: codeController,
                textCapitalization: TextCapitalization.characters,
                decoration: const InputDecoration(
                  border: OutlineInputBorder(),
                  labelText: 'Session code',
                  hintText: 'AUDIO-7F2K',
                ),
              ),
            const SizedBox(height: 16),
            FilledButton.icon(
              onPressed: onJoin,
              icon: const Icon(Icons.login),
              label: const Text('Join session'),
            ),
          ],
        ),
      ),
    );
  }
}

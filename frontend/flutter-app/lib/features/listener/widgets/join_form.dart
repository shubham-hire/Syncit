import 'package:flutter/material.dart';

class JoinForm extends StatelessWidget {
  const JoinForm({required this.codeController, required this.onJoin, super.key});

  final TextEditingController codeController;
  final VoidCallback onJoin;

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
              style: Theme.of(context).textTheme.titleLarge?.copyWith(
                fontWeight: FontWeight.w700,
              ),
            ),
            const SizedBox(height: 12),
            const Text(
              'Connect to the host hotspot, then scan its QR code or enter the short session code below.',
            ),
            const SizedBox(height: 20),
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

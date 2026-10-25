import 'package:flutter/material.dart';

import '../../session/session_constants.dart';

class SessionCodeCard extends StatelessWidget {
  const SessionCodeCard({this.sessionHex, this.tokenHex, super.key});

  /// Live engine values; null keeps the mock session code.
  final String? sessionHex;
  final String? tokenHex;

  @override
  Widget build(BuildContext context) {
    final code = sessionHex ?? mockSessionCode;
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(20),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              'Join information',
              style: Theme.of(
                context,
              ).textTheme.titleLarge?.copyWith(fontWeight: FontWeight.w700),
            ),
            const SizedBox(height: 16),
            Container(
              width: double.infinity,
              padding: const EdgeInsets.symmetric(vertical: 24),
              decoration: BoxDecoration(
                color: Theme.of(context).colorScheme.surfaceContainerHighest,
                borderRadius: BorderRadius.circular(12),
              ),
              child: Column(
                children: [
                  const Icon(Icons.qr_code_2, size: 96),
                  const SizedBox(height: 12),
                  SelectableText(
                    code,
                    style: TextStyle(
                      fontSize: sessionHex == null ? 24 : 16,
                      fontFamily: sessionHex == null ? null : 'monospace',
                    ),
                    textAlign: TextAlign.center,
                  ),
                  const SizedBox(height: 4),
                  if (sessionHex == null)
                    const Text('QR generation will use the live session token.')
                  else ...[
                    SelectableText(
                      'token ${tokenHex ?? '-'}',
                      style: const TextStyle(fontFamily: 'monospace'),
                      textAlign: TextAlign.center,
                    ),
                    const Text(
                      'Share the host address, session and token with listeners.',
                    ),
                  ],
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}

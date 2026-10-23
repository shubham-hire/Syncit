import 'package:flutter/material.dart';

import '../../session/session_constants.dart';

class SessionCodeCard extends StatelessWidget {
  const SessionCodeCard({super.key});

  @override
  Widget build(BuildContext context) {
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
              child: const Column(
                children: [
                  Icon(Icons.qr_code_2, size: 96),
                  SizedBox(height: 12),
                  Text(mockSessionCode, style: TextStyle(fontSize: 24)),
                  SizedBox(height: 4),
                  Text('QR generation will use the live session token.'),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}

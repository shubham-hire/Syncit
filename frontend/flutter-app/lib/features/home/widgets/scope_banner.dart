import 'package:flutter/material.dart';

class ScopeBanner extends StatelessWidget {
  const ScopeBanner({super.key});

  @override
  Widget build(BuildContext context) {
    return Card(
      color: Theme.of(context).colorScheme.secondaryContainer,
      child: const Padding(
        padding: EdgeInsets.all(16),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Icon(Icons.info_outline),
            SizedBox(width: 12),
            Expanded(
              child: Text(
                'Initial target: Windows and Android. Apple devices are outside the project scope.',
              ),
            ),
          ],
        ),
      ),
    );
  }
}

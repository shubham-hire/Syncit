import 'package:flutter/material.dart';

class HotspotInstructions extends StatelessWidget {
  const HotspotInstructions({super.key});

  @override
  Widget build(BuildContext context) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(20),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              'Host hotspot',
              style: Theme.of(
                context,
              ).textTheme.titleLarge?.copyWith(fontWeight: FontWeight.w700),
            ),
            const SizedBox(height: 12),
            const Text(
              'Enable the hotspot on this device. Listeners must connect to this network; they should not create their own hotspot.',
            ),
            const SizedBox(height: 12),
            const _ChecklistItem(text: 'Use a private local network.'),
            const _ChecklistItem(text: 'Prefer 5 GHz when available.'),
            const _ChecklistItem(
              text: 'Check that connected devices can reach the host.',
            ),
          ],
        ),
      ),
    );
  }
}

class _ChecklistItem extends StatelessWidget {
  const _ChecklistItem({required this.text});

  final String text;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 6),
      child: Row(
        children: [
          const Icon(Icons.check, size: 18),
          const SizedBox(width: 8),
          Expanded(child: Text(text)),
        ],
      ),
    );
  }
}

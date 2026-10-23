import 'package:flutter/material.dart';

class ListenerList extends StatelessWidget {
  const ListenerList({
    required this.listeners,
    required this.approvedListeners,
    required this.onSimulateListener,
    required this.onApprove,
    required this.onRemove,
    super.key,
  });

  final List<String> listeners;
  final Set<String> approvedListeners;
  final VoidCallback onSimulateListener;
  final ValueChanged<String> onApprove;
  final ValueChanged<String> onRemove;

  @override
  Widget build(BuildContext context) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(20),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: [
                Text(
                  'Listeners',
                  style: Theme.of(
                    context,
                  ).textTheme.titleLarge?.copyWith(fontWeight: FontWeight.w700),
                ),
                Text('${listeners.length}/5'),
              ],
            ),
            const SizedBox(height: 12),
            if (listeners.isEmpty)
              const Text('No listeners have joined yet.')
            else
              ...listeners.map(
                (listener) => ListTile(
                  contentPadding: EdgeInsets.zero,
                  leading: const Icon(Icons.headphones),
                  title: Text(listener),
                  subtitle: Text(
                    approvedListeners.contains(listener)
                        ? 'Approved · mock transport'
                        : 'Awaiting host approval',
                  ),
                  trailing: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      if (!approvedListeners.contains(listener))
                        IconButton(
                          tooltip: 'Approve listener',
                          onPressed: () => onApprove(listener),
                          icon: const Icon(Icons.check_circle_outline),
                        ),
                      IconButton(
                        tooltip: 'Remove listener',
                        onPressed: () => onRemove(listener),
                        icon: const Icon(Icons.remove_circle_outline),
                      ),
                    ],
                  ),
                ),
              ),
            const SizedBox(height: 8),
            OutlinedButton.icon(
              onPressed: listeners.length < 5 ? onSimulateListener : null,
              icon: const Icon(Icons.person_add_alt_1),
              label: const Text('Simulate listener'),
            ),
          ],
        ),
      ),
    );
  }
}

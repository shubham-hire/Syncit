import 'package:flutter/material.dart';

import '../theme/app_theme.dart';

/// Apple Settings-style status row: tinted icon badge, strong title,
/// quiet message. The optional section label renders outside the card.
class StatusCard extends StatelessWidget {
  const StatusCard({
    required this.icon,
    required this.title,
    required this.message,
    this.action,
    this.sectionLabel,
    this.iconTint,
    super.key,
  });

  final IconData icon;
  final String title;
  final String message;
  final Widget? action;
  final String? sectionLabel;
  final Color? iconTint;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final card = Card(
      child: Padding(
        padding: const EdgeInsets.all(18),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Container(
              width: 38,
              height: 38,
              decoration: BoxDecoration(
                color: (iconTint ?? scheme.primary).withValues(alpha: 0.12),
                borderRadius: BorderRadius.circular(11),
              ),
              child: Icon(icon, size: 20, color: iconTint ?? scheme.primary),
            ),
            const SizedBox(width: 14),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    title,
                    style: AppType.title.copyWith(color: scheme.onSurface),
                  ),
                  const SizedBox(height: 5),
                  Text(
                    message,
                    style: AppType.body.copyWith(
                      color: scheme.onSurface.withValues(alpha: 0.72),
                    ),
                  ),
                  if (action != null) ...[const SizedBox(height: 14), action!],
                ],
              ),
            ),
          ],
        ),
      ),
    );

    final label = sectionLabel;
    if (label == null) return card;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          label.toUpperCase(),
          style: AppType.sectionLabel.copyWith(
            color: scheme.onSurface.withValues(alpha: 0.55),
          ),
        ),
        const SizedBox(height: 8),
        card,
      ],
    );
  }
}

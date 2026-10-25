import 'package:flutter/material.dart';

import '../../../core/theme/liquid_glass.dart';

class RoleCard extends StatelessWidget {
  const RoleCard({
    required this.icon,
    required this.title,
    required this.description,
    required this.buttonLabel,
    required this.onPressed,
    super.key,
  });

  final IconData icon;
  final String title;
  final String description;
  final String buttonLabel;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    // Role selection is a choice, not dense content: it earns the glass
    // treatment with instant press feedback.
    return PressableGlass(
      borderRadius: 20,
      onPressed: onPressed,
      child: LiquidGlass(
        borderRadius: 20,
        child: Padding(
          padding: const EdgeInsets.all(20),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Icon(icon, size: 32),
              const SizedBox(width: 16),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      title,
                      // Vibrancy: heavier text over glass.
                      style: Theme.of(context).textTheme.titleLarge?.copyWith(
                        fontWeight: FontWeight.w700,
                      ),
                    ),
                    const SizedBox(height: 8),
                    Text(description),
                    const SizedBox(height: 16),
                    FilledButton.tonal(
                      onPressed: onPressed,
                      child: Text(buttonLabel),
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

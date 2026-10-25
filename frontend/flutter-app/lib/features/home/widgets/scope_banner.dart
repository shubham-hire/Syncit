import 'package:flutter/material.dart';

import '../../../core/theme/app_theme.dart';
import '../../../core/theme/liquid_glass.dart';

class ScopeBanner extends StatelessWidget {
  const ScopeBanner({super.key});

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return LiquidGlass(
      weight: LiquidGlassWeight.thin,
      borderRadius: 16,
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 12),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Icon(Icons.info_outline, size: 16, color: scheme.primary),
            const SizedBox(width: 10),
            Expanded(
              child: Text(
                'Initial target: Windows and Android. Apple devices are outside the project scope.',
                style: AppType.body.copyWith(
                  color: scheme.onSurface.withValues(alpha: 0.72),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

import 'package:flutter/material.dart';

import '../../../core/theme/app_theme.dart';
import '../../../core/theme/liquid_glass.dart';

class PlaybackControls extends StatelessWidget {
  const PlaybackControls({
    required this.delay,
    required this.volume,
    required this.muted,
    required this.onDelayChanged,
    required this.onVolumeChanged,
    required this.onMuteChanged,
    super.key,
  });

  final double delay;
  final double volume;
  final bool muted;
  final ValueChanged<double> onDelayChanged;
  final ValueChanged<double> onVolumeChanged;
  final ValueChanged<bool> onMuteChanged;

  @override
  Widget build(BuildContext context) {
    return LiquidGlass(
      weight: LiquidGlassWeight.chrome,
      child: Padding(
        padding: const EdgeInsets.all(20),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              'PLAYBACK',
              style: AppType.sectionLabel.copyWith(
                color: Theme.of(
                  context,
                ).colorScheme.onSurface.withValues(alpha: 0.75),
              ),
            ),
            const SizedBox(height: 16),
            Row(
              children: [
                const Icon(Icons.tune),
                const SizedBox(width: 12),
                Expanded(child: Text('Delay ${delay.round()} ms')),
              ],
            ),
            Slider(
              value: delay,
              min: -300,
              max: 300,
              divisions: 60,
              label: '${delay.round()} ms',
              onChanged: onDelayChanged,
            ),
            Row(
              children: [
                Icon(muted ? Icons.volume_off : Icons.volume_up),
                const SizedBox(width: 12),
                Expanded(child: Text('Volume ${(volume * 100).round()}%')),
                Switch(value: muted, onChanged: onMuteChanged),
              ],
            ),
            Slider(
              value: volume,
              min: 0,
              max: 1,
              divisions: 20,
              label: '${(volume * 100).round()}%',
              onChanged: muted ? null : onVolumeChanged,
            ),
            const SizedBox(height: 8),
            const Text(
              'The delay control compensates for different wired and Bluetooth audio paths.',
            ),
          ],
        ),
      ),
    );
  }
}

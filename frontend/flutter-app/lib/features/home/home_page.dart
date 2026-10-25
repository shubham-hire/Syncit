import 'package:flutter/material.dart';

import '../../core/theme/app_theme.dart';
import '../../core/theme/liquid_glass.dart';
import '../host/host_session_page.dart';
import '../listener/listener_join_page.dart';
import 'widgets/role_card.dart';
import 'widgets/scope_banner.dart';

class HomePage extends StatelessWidget {
  const HomePage({super.key});

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return GlassScaffold(
      title: 'AudioSync',
      largeTitle: 'AudioSync',
      subtitle:
          'Share one local audio session with your own devices and headphones.',
      children: [
        GlassChip(
          icon: Icons.wifi_tethering,
          label: 'PROTOTYPE · LOCAL NETWORK',
        ),
        const SizedBox(height: 12),
        const ScopeBanner(),
        const SizedBox(height: 28),
        Text(
          'CHOOSE A ROLE',
          style: AppType.sectionLabel.copyWith(
            color: scheme.onSurface.withValues(alpha: 0.55),
          ),
        ),
        const SizedBox(height: 12),
        RoleCard(
          icon: Icons.wifi_tethering,
          title: 'Host a session',
          description:
              'Share audio from this device over its local Wi-Fi hotspot.',
          buttonLabel: 'Create session',
          onPressed: () {
            Navigator.of(context).push(
              MaterialPageRoute<void>(builder: (_) => const HostSessionPage()),
            );
          },
        ),
        const SizedBox(height: 16),
        RoleCard(
          icon: Icons.headphones,
          title: 'Listen to a session',
          description:
              'Join a host and play the shared audio through this device.',
          buttonLabel: 'Join session',
          onPressed: () {
            Navigator.of(context).push(
              MaterialPageRoute<void>(builder: (_) => const ListenerJoinPage()),
            );
          },
        ),
        const SizedBox(height: 28),
        Text(
          'PROTOTYPE STATUS',
          style: AppType.sectionLabel.copyWith(
            color: scheme.onSurface.withValues(alpha: 0.55),
          ),
        ),
        const SizedBox(height: 8),
        Text(
          'The host/listener flow now drives the real Rust engine when its library is present, and falls back to a mock session otherwise. Real hotspot capture and Android playback are the next milestones.',
          style: AppType.body.copyWith(
            color: scheme.onSurface.withValues(alpha: 0.72),
          ),
        ),
      ],
    );
  }
}

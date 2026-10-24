import 'package:flutter/material.dart';

import '../host/host_session_page.dart';
import '../listener/listener_join_page.dart';
import 'widgets/role_card.dart';
import 'widgets/scope_banner.dart';

class HomePage extends StatelessWidget {
  const HomePage({super.key});

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: SafeArea(
        child: Center(
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 720),
            child: SingleChildScrollView(
              padding: const EdgeInsets.all(24),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  const SizedBox(height: 24),
                  Text(
                    'AudioSync',
                    style: Theme.of(context).textTheme.displaySmall?.copyWith(
                      fontWeight: FontWeight.w700,
                    ),
                  ),
                  const SizedBox(height: 8),
                  Text(
                    'Share one local audio session with your own devices and headphones.',
                    style: Theme.of(context).textTheme.titleMedium,
                  ),
                  const SizedBox(height: 12),
                  const ScopeBanner(),
                  const SizedBox(height: 32),
                  Text(
                    'Choose a role',
                    style: Theme.of(context).textTheme.headlineSmall,
                  ),
                  const SizedBox(height: 16),
                  RoleCard(
                    icon: Icons.wifi_tethering,
                    title: 'Host a session',
                    description:
                        'Share audio from this device over its local Wi-Fi hotspot.',
                    buttonLabel: 'Create session',
                    onPressed: () {
                      Navigator.of(context).push(
                        MaterialPageRoute<void>(
                          builder: (_) => const HostSessionPage(),
                        ),
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
                        MaterialPageRoute<void>(
                          builder: (_) => const ListenerJoinPage(),
                        ),
                      );
                    },
                  ),
                  const SizedBox(height: 32),
                  Text(
                    'Prototype status',
                    style: Theme.of(context).textTheme.titleMedium?.copyWith(
                      fontWeight: FontWeight.w700,
                    ),
                  ),
                  const SizedBox(height: 8),
                  const Text(
                    'The current build contains the host/listener flow and a mock session. Real Windows capture, Opus transport and Android playback are the next integration milestones.',
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}

import 'package:flutter/material.dart';

const mockSessionCode = 'AUDIO-7F2K';

void main() {
  runApp(const AudioSyncApp());
}

class AudioSyncApp extends StatelessWidget {
  const AudioSyncApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'AudioSync',
      debugShowCheckedModeBanner: false,
      theme: ThemeData(
        colorScheme: ColorScheme.fromSeed(seedColor: const Color(0xFF3157D5)),
        useMaterial3: true,
      ),
      home: const HomePage(),
    );
  }
}

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
                    style: Theme.of(context).textTheme.displaySmall
                        ?.copyWith(fontWeight: FontWeight.w700),
                  ),
                  const SizedBox(height: 8),
                  Text(
                    'Share one local audio session with your own devices and headphones.',
                    style: Theme.of(context).textTheme.titleMedium,
                  ),
                  const SizedBox(height: 12),
                  const _ScopeBanner(),
                  const SizedBox(height: 32),
                  Text(
                    'Choose a role',
                    style: Theme.of(context).textTheme.headlineSmall,
                  ),
                  const SizedBox(height: 16),
                  _RoleCard(
                    icon: Icons.wifi_tethering,
                    title: 'Host a session',
                    description: 'Share audio from this device over its local Wi-Fi hotspot.',
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
                  _RoleCard(
                    icon: Icons.headphones,
                    title: 'Listen to a session',
                    description: 'Join a host and play the shared audio through this device.',
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
                    style: Theme.of(context).textTheme.titleMedium
                        ?.copyWith(fontWeight: FontWeight.w700),
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

class _ScopeBanner extends StatelessWidget {
  const _ScopeBanner();

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

class _RoleCard extends StatelessWidget {
  const _RoleCard({
    required this.icon,
    required this.title,
    required this.description,
    required this.buttonLabel,
    required this.onPressed,
  });

  final IconData icon;
  final String title;
  final String description;
  final String buttonLabel;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    return Card(
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
                    style: Theme.of(context).textTheme.titleLarge
                        ?.copyWith(fontWeight: FontWeight.w700),
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
    );
  }
}

class HostSessionPage extends StatefulWidget {
  const HostSessionPage({super.key});

  @override
  State<HostSessionPage> createState() => _HostSessionPageState();
}

class _HostSessionPageState extends State<HostSessionPage> {
  bool _sessionStarted = false;
  final List<String> _listeners = <String>[];
  final Set<String> _approvedListeners = <String>{};

  void _startSession() {
    setState(() => _sessionStarted = true);
  }

  void _addMockListener() {
    setState(() {
      _listeners.add('Listener ${_listeners.length + 1}');
    });
  }

  void _approveListener(String listener) {
    setState(() => _approvedListeners.add(listener));
  }

  void _removeListener(String listener) {
    setState(() {
      _listeners.remove(listener);
      _approvedListeners.remove(listener);
    });
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Host session')),
      body: Center(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 720),
          child: ListView(
            padding: const EdgeInsets.all(24),
            children: [
              _StatusCard(
                icon: _sessionStarted ? Icons.play_circle : Icons.pause_circle,
                title: _sessionStarted
                    ? 'Session is ready'
                    : 'Session not started',
                message: _sessionStarted
                    ? 'Listeners can join using the session information below.'
                    : 'Start a local session after enabling the host hotspot.',
                action: _sessionStarted
                    ? null
                    : FilledButton(
                        onPressed: _startSession,
                        child: const Text('Start session'),
                      ),
              ),
              const SizedBox(height: 16),
              const _HotspotInstructions(),
              if (_sessionStarted) ...[
                const SizedBox(height: 16),
                const _SessionCodeCard(),
                const SizedBox(height: 16),
                _ListenerList(
                  listeners: _listeners,
                  approvedListeners: _approvedListeners,
                  onSimulateListener: _addMockListener,
                  onApprove: _approveListener,
                  onRemove: _removeListener,
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}

class _HotspotInstructions extends StatelessWidget {
  const _HotspotInstructions();

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
              style: Theme.of(context).textTheme.titleLarge
                  ?.copyWith(fontWeight: FontWeight.w700),
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

class _SessionCodeCard extends StatelessWidget {
  const _SessionCodeCard();

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
              style: Theme.of(context).textTheme.titleLarge
                  ?.copyWith(fontWeight: FontWeight.w700),
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

class _ListenerList extends StatelessWidget {
  const _ListenerList({
    required this.listeners,
    required this.approvedListeners,
    required this.onSimulateListener,
    required this.onApprove,
    required this.onRemove,
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
                  style: Theme.of(context).textTheme.titleLarge
                      ?.copyWith(fontWeight: FontWeight.w700),
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

class ListenerJoinPage extends StatefulWidget {
  const ListenerJoinPage({super.key});

  @override
  State<ListenerJoinPage> createState() => _ListenerJoinPageState();
}

class _ListenerJoinPageState extends State<ListenerJoinPage> {
  final TextEditingController _codeController = TextEditingController();
  bool _joined = false;
  double _delay = 0;
  double _volume = 0.8;
  bool _muted = false;

  @override
  void dispose() {
    _codeController.dispose();
    super.dispose();
  }

  void _join() {
    final code = _codeController.text.trim().toUpperCase();
    if (code.isEmpty) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Enter the session code first.')),
      );
      return;
    }
    if (code != mockSessionCode) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('That mock session code is not valid.')),
      );
      return;
    }
    setState(() => _joined = true);
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Listen to a session')),
      body: Center(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 720),
          child: ListView(
            padding: const EdgeInsets.all(24),
            children: [
              if (!_joined)
                _JoinForm(codeController: _codeController, onJoin: _join),
              if (_joined) ...[
                const _StatusCard(
                  icon: Icons.check_circle,
                  title: 'Mock session connected',
                  message: 'Playback controls are ready for the real listener engine.',
                ),
                const SizedBox(height: 16),
                _PlaybackControls(
                  delay: _delay,
                  volume: _volume,
                  muted: _muted,
                  onDelayChanged: (value) => setState(() => _delay = value),
                  onVolumeChanged: (value) => setState(() => _volume = value),
                  onMuteChanged: (value) => setState(() => _muted = value),
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}

class _JoinForm extends StatelessWidget {
  const _JoinForm({required this.codeController, required this.onJoin});

  final TextEditingController codeController;
  final VoidCallback onJoin;

  @override
  Widget build(BuildContext context) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(20),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              'Join a host',
              style: Theme.of(context).textTheme.titleLarge
                  ?.copyWith(fontWeight: FontWeight.w700),
            ),
            const SizedBox(height: 12),
            const Text(
              'Connect to the host hotspot, then scan its QR code or enter the short session code below.',
            ),
            const SizedBox(height: 20),
            TextField(
              controller: codeController,
              textCapitalization: TextCapitalization.characters,
              decoration: const InputDecoration(
                border: OutlineInputBorder(),
                labelText: 'Session code',
                hintText: 'AUDIO-7F2K',
              ),
            ),
            const SizedBox(height: 16),
            FilledButton.icon(
              onPressed: onJoin,
              icon: const Icon(Icons.login),
              label: const Text('Join session'),
            ),
          ],
        ),
      ),
    );
  }
}

class _PlaybackControls extends StatelessWidget {
  const _PlaybackControls({
    required this.delay,
    required this.volume,
    required this.muted,
    required this.onDelayChanged,
    required this.onVolumeChanged,
    required this.onMuteChanged,
  });

  final double delay;
  final double volume;
  final bool muted;
  final ValueChanged<double> onDelayChanged;
  final ValueChanged<double> onVolumeChanged;
  final ValueChanged<bool> onMuteChanged;

  @override
  Widget build(BuildContext context) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(20),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              'Playback',
              style: Theme.of(context).textTheme.titleLarge
                  ?.copyWith(fontWeight: FontWeight.w700),
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

class _StatusCard extends StatelessWidget {
  const _StatusCard({
    required this.icon,
    required this.title,
    required this.message,
    this.action,
  });

  final IconData icon;
  final String title;
  final String message;
  final Widget? action;

  @override
  Widget build(BuildContext context) {
    return Card(
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
                    style: Theme.of(context).textTheme.titleLarge
                        ?.copyWith(fontWeight: FontWeight.w700),
                  ),
                  const SizedBox(height: 8),
                  Text(message),
                  if (action != null) ...[const SizedBox(height: 16), action!],
                ],
              ),
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

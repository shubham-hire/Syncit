import 'package:flutter/material.dart';

import 'features/home/home_page.dart';

class SyncItApp extends StatelessWidget {
  const SyncItApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'SyncIt',
      debugShowCheckedModeBanner: false,
      theme: ThemeData(
        colorScheme: ColorScheme.fromSeed(seedColor: const Color(0xFFB51F2B)),
        useMaterial3: true,
        // InkSparkle's runtime shader is not available in every Flutter test
        // engine, so use the portable ripple implementation for all targets.
        splashFactory: InkRipple.splashFactory,
      ),
      home: const HomePage(),
    );
  }
}

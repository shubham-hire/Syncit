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
      ),
      home: const HomePage(),
    );
  }
}

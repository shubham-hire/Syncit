import 'package:flutter/material.dart';

/// AudioSync theme: warm paper surfaces, graphite ink, and the vermilion
/// signal from the brand palette. Content cards stay quiet and solid; glass
/// is reserved for chrome and controls (see [LiquidGlass]).
class AppTheme {
  const AppTheme._();

  static const Color vermilion = Color(0xFFB51F2B);

  static ThemeData light() {
    final scheme = ColorScheme.fromSeed(
      seedColor: vermilion,
      brightness: Brightness.light,
    );
    return ThemeData(
      colorScheme: scheme,
      useMaterial3: true,
      // InkSparkle's runtime shader is not available in every Flutter test
      // engine, so use the portable ripple implementation for all targets.
      splashFactory: InkRipple.splashFactory,
      scaffoldBackgroundColor: scheme.surfaceContainerLowest,
      appBarTheme: const AppBarTheme(
        backgroundColor: Colors.transparent,
        elevation: 0,
        scrolledUnderElevation: 0,
        centerTitle: false,
      ),
      cardTheme: CardThemeData(
        color: scheme.surfaceContainerLow,
        elevation: 0,
        surfaceTintColor: Colors.transparent,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(20),
          side: BorderSide(color: scheme.outlineVariant.withValues(alpha: 0.5)),
        ),
        margin: EdgeInsets.zero,
      ),
      dividerTheme: DividerThemeData(
        color: scheme.outlineVariant.withValues(alpha: 0.5),
        space: 1,
      ),
      snackBarTheme: SnackBarThemeData(
        behavior: SnackBarBehavior.floating,
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(14)),
      ),
    );
  }
}

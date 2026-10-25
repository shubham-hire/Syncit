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

/// Apple-style typography: tracking is size-specific (tight on display,
/// open on small caps), leading tracks size inversely.
class AppType {
  const AppType._();

  /// Large-title / hero: tight tracking, strong weight.
  static const TextStyle display = TextStyle(
    fontSize: 34,
    fontWeight: FontWeight.w800,
    letterSpacing: -0.8,
    height: 1.08,
  );

  /// Card titles.
  static const TextStyle title = TextStyle(
    fontSize: 17,
    fontWeight: FontWeight.w700,
    letterSpacing: -0.2,
  );

  /// Section headers: small caps with open tracking.
  static const TextStyle sectionLabel = TextStyle(
    fontSize: 11,
    fontWeight: FontWeight.w600,
    letterSpacing: 1.1,
  );

  /// Body copy.
  static const TextStyle body = TextStyle(
    fontSize: 14.5,
    fontWeight: FontWeight.w400,
    height: 1.45,
  );

  /// Hero card titles.
  static const TextStyle heroTitle = TextStyle(
    fontSize: 21,
    fontWeight: FontWeight.w700,
    letterSpacing: -0.4,
  );
}

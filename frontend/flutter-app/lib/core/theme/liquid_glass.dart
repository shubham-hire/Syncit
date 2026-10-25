import 'dart:ui';

import 'package:flutter/material.dart';

/// Apple-style Liquid Glass material system.
///
/// Follows the Apple material rules the [apple-design] skill distills:
///
/// - Glass is a *chrome* material: it hosts navigation, floating controls,
///   and hero panels — never dense content lists, and never another glass
///   surface (stacking translucent-on-translucent collapses legibility).
/// - The material catches light on its top edge (a bright sheen that fades
///   downward) and separates with a soft, size-scaled shadow.
/// - Bigger surfaces read as thicker: stronger blur and a deeper shadow.
/// - Text over glass is slightly heavier (vibrancy), not flat gray.
/// - Feedback lives on pointer-down, and animates briefly (interruptible);
///   reduced-motion users get instant states instead.
///
/// Implemented with stable Flutter primitives (`BackdropFilter`,
/// `ImageFilter.compose`, `ColorFilter.matrix`) so it works on Windows and
/// Android without experimental APIs or new dependencies.

/// Visual weight of a glass surface. Bigger = thicker material.
enum LiquidGlassWeight { thin, regular, thick, chrome }

/// Resolved material constants for one glass surface.
@immutable
class LiquidGlassFrost {
  const LiquidGlassFrost({
    required this.tint,
    required this.borderColor,
    required this.sheenColor,
    required this.shadowColor,
    required this.shadowBlur,
    required this.shadowOffset,
    required this.blurSigma,
  });

  final Color tint;
  final Color borderColor;
  final Color sheenColor;
  final Color shadowColor;
  final double shadowBlur;
  final Offset shadowOffset;
  final double blurSigma;
}

/// Backdrop blur + saturation filter for a frost weight. Extracted so the
/// same filter can be reused without rebuilding matrices every frame.
ImageFilter liquidGlassFilter(double blurSigma) => ImageFilter.compose(
  outer: ImageFilter.blur(sigmaX: blurSigma, sigmaY: blurSigma),
  inner: ColorFilter.matrix(_saturationMatrix(1.8)),
);

/// Standard saturation color matrix (`s` = 1 leaves colors unchanged).
List<double> _saturationMatrix(double s) => <double>[
  0.2126 + 0.7874 * s,
  0.7152 - 0.7152 * s,
  0.0722 - 0.0722 * s,
  0,
  0, //
  0.2126 - 0.2126 * s,
  0.7152 + 0.2848 * s,
  0.0722 - 0.0722 * s,
  0,
  0, //
  0.2126 - 0.2126 * s,
  0.7152 - 0.7152 * s,
  0.0722 + 0.9278 * s,
  0,
  0, //
  0,
  0,
  0,
  1,
  0,
];

/// Per-weight, per-brightness material constants.
LiquidGlassFrost frostFor(
  Brightness brightness,
  LiquidGlassWeight weight,
) {
  final dark = brightness == Brightness.dark;
  return switch (weight) {
    LiquidGlassWeight.thin => LiquidGlassFrost(
      tint: (dark ? const Color(0xFF1C1C22) : Colors.white).withValues(
        alpha: dark ? 0.50 : 0.58,
      ),
      borderColor: Colors.white.withValues(alpha: dark ? 0.14 : 0.60),
      sheenColor: Colors.white.withValues(alpha: dark ? 0.12 : 0.45),
      shadowColor: Colors.black.withValues(alpha: dark ? 0.55 : 0.16),
      shadowBlur: 10,
      shadowOffset: const Offset(0, 4),
      blurSigma: 14,
    ),
    LiquidGlassWeight.regular => LiquidGlassFrost(
      tint: (dark ? const Color(0xFF1C1C22) : Colors.white).withValues(
        alpha: dark ? 0.55 : 0.64,
      ),
      borderColor: Colors.white.withValues(alpha: dark ? 0.16 : 0.65),
      sheenColor: Colors.white.withValues(alpha: dark ? 0.14 : 0.50),
      shadowColor: Colors.black.withValues(alpha: dark ? 0.55 : 0.18),
      shadowBlur: 18,
      shadowOffset: const Offset(0, 8),
      blurSigma: 18,
    ),
    LiquidGlassWeight.thick => LiquidGlassFrost(
      tint: (dark ? const Color(0xFF1C1C22) : Colors.white).withValues(
        alpha: dark ? 0.62 : 0.72,
      ),
      borderColor: Colors.white.withValues(alpha: dark ? 0.18 : 0.70),
      sheenColor: Colors.white.withValues(alpha: dark ? 0.16 : 0.55),
      shadowColor: Colors.black.withValues(alpha: dark ? 0.60 : 0.22),
      shadowBlur: 30,
      shadowOffset: const Offset(0, 12),
      blurSigma: 24,
    ),
    LiquidGlassWeight.chrome => LiquidGlassFrost(
      tint: (dark ? const Color(0xFF1C1C22) : Colors.white).withValues(
        alpha: dark ? 0.55 : 0.62,
      ),
      borderColor: Colors.white.withValues(alpha: dark ? 0.16 : 0.65),
      sheenColor: Colors.white.withValues(alpha: dark ? 0.14 : 0.50),
      shadowColor: Colors.black.withValues(alpha: dark ? 0.55 : 0.18),
      shadowBlur: 22,
      shadowOffset: const Offset(0, 8),
      blurSigma: 22,
    ),
  };
}

/// A translucent Liquid Glass surface: blurs and saturates whatever scrolls
/// behind it, catches light on the top edge, and lifts on a soft shadow.
class LiquidGlass extends StatelessWidget {
  const LiquidGlass({
    required this.child,
    this.weight = LiquidGlassWeight.regular,
    this.borderRadius = 20,
    this.padding,
    this.width,
    this.height,
    super.key,
  });

  final Widget child;
  final LiquidGlassWeight weight;
  final double borderRadius;
  final EdgeInsetsGeometry? padding;
  final double? width;
  final double? height;

  @override
  Widget build(BuildContext context) {
    final frost = frostFor(Theme.of(context).brightness, weight);
    final radius = BorderRadius.circular(borderRadius);

    Widget surface = DecoratedBox(
      decoration: BoxDecoration(
        borderRadius: radius,
        boxShadow: [
          BoxShadow(
            color: frost.shadowColor,
            blurRadius: frost.shadowBlur,
            offset: frost.shadowOffset,
          ),
        ],
      ),
      child: ClipRRect(
        borderRadius: radius,
        child: BackdropFilter(
          filter: liquidGlassFilter(frost.blurSigma),
          child: Container(
            padding: padding,
            decoration: BoxDecoration(color: frost.tint),
            // Light catches the top edge of the material and fades down.
            foregroundDecoration: BoxDecoration(
              borderRadius: radius,
              border: Border.all(color: frost.borderColor, width: 1),
              gradient: LinearGradient(
                begin: Alignment.topCenter,
                end: Alignment.bottomCenter,
                colors: [
                  frost.sheenColor,
                  frost.sheenColor.withValues(alpha: 0),
                ],
                stops: const [0, 0.55],
              ),
            ),
            child: child,
          ),
        ),
      ),
    );

    if (width != null || height != null) {
      surface = SizedBox(width: width, height: height, child: surface);
    }
    return surface;
  }
}

/// Press feedback that begins on pointer-down (Apple: respond instantly, not
/// on release). The scale animation is short and interruptible; reduced-motion
/// users get instant states.
class PressableGlass extends StatefulWidget {
  const PressableGlass({
    required this.child,
    required this.onPressed,
    this.borderRadius = 20,
    super.key,
  });

  final Widget child;
  final VoidCallback onPressed;
  final double borderRadius;

  @override
  State<PressableGlass> createState() => _PressableGlassState();
}

class _PressableGlassState extends State<PressableGlass> {
  bool _pressed = false;

  void _setPressed(bool pressed) {
    if (_pressed != pressed) setState(() => _pressed = pressed);
  }

  @override
  Widget build(BuildContext context) {
    final instant = MediaQuery.of(context).disableAnimations;
    return GestureDetector(
      behavior: HitTestBehavior.opaque,
      onTap: widget.onPressed,
      onTapDown: (_) => _setPressed(true),
      onTapCancel: () => _setPressed(false),
      onTapUp: (_) => _setPressed(false),
      child: AnimatedScale(
        scale: _pressed ? 0.97 : 1.0,
        duration: instant ? Duration.zero : const Duration(milliseconds: 100),
        curve: Curves.easeOut,
        child: widget.child,
      ),
    );
  }
}

/// Page scaffold with the shared backdrop: a soft vertical surface gradient
/// plus two faint brand color washes, so glass has real content to refract
/// behind it. The app bar is a floating glass chrome layer that page content
/// scrolls under.
class GlassScaffold extends StatelessWidget {
  const GlassScaffold({required this.title, required this.children, super.key});

  final String title;
  final List<Widget> children;

  static const double _barHeight = 72;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final canPop = Navigator.of(context).canPop();

    return Scaffold(
      backgroundColor: Colors.transparent,
      extendBodyBehindAppBar: true,
      appBar: PreferredSize(
        preferredSize: const Size.fromHeight(_barHeight),
        child: SafeArea(
          bottom: false,
          child: Padding(
            padding: const EdgeInsets.fromLTRB(16, 10, 16, 10),
            child: LiquidGlass(
              weight: LiquidGlassWeight.chrome,
              borderRadius: 30,
              child: SizedBox(
                height: 52,
                child: Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 8),
                  child: Row(
                    children: [
                      if (canPop)
                        IconButton(
                          onPressed: () => Navigator.of(context).maybePop(),
                          icon: const Icon(Icons.arrow_back_rounded),
                        ),
                      const SizedBox(width: 8),
                      Expanded(
                        child: Text(
                          title,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          // Vibrancy: slightly heavier text over glass.
                          style: Theme.of(context).textTheme.titleLarge
                              ?.copyWith(fontWeight: FontWeight.w700),
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
      body: DecoratedBox(
        decoration: BoxDecoration(
          gradient: LinearGradient(
            begin: Alignment.topCenter,
            end: Alignment.bottomCenter,
            colors: [
              scheme.surfaceContainerLowest,
              scheme.surfaceContainerLow,
            ],
          ),
        ),
        child: Stack(
          fit: StackFit.expand,
          children: [
            // Faint brand washes give the glass something to refract.
            IgnorePointer(
              child: DecoratedBox(
                decoration: BoxDecoration(
                  gradient: RadialGradient(
                    center: const Alignment(0.95, -0.85),
                    radius: 0.9,
                    colors: [
                      scheme.primary.withValues(alpha: 0.10),
                      scheme.primary.withValues(alpha: 0),
                    ],
                  ),
                ),
              ),
            ),
            IgnorePointer(
              child: DecoratedBox(
                decoration: BoxDecoration(
                  gradient: RadialGradient(
                    center: const Alignment(-0.9, 0.95),
                    radius: 0.9,
                    colors: [
                      scheme.secondaryContainer.withValues(alpha: 0.35),
                      scheme.secondaryContainer.withValues(alpha: 0),
                    ],
                  ),
                ),
              ),
            ),
            ListView(
              padding: const EdgeInsets.fromLTRB(
                24,
                _barHeight + 28,
                24,
                32,
              ),
              children: children,
            ),
          ],
        ),
      ),
    );
  }
}

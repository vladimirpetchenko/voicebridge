import 'package:flutter/material.dart';

/// Тема, повторяющая брендинг десктопа: палитра + шрифт Fira Code.
class AppTheme {
  static const bg = Color(0xFF0F1218);
  static const surface = Color(0xFF171B23);
  static const surface2 = Color(0xFF1E242F);
  static const accent = Color(0xFF22D3EE);
  static const accent2 = Color(0xFF38BDF8);
  static const textPrimary = Color(0xFFE6EBF2);
  static const textDim = Color(0xFF8A94A6);
  static const danger = Color(0xFFF87171);

  static const fontFamily = 'FiraCode';

  static ThemeData dark() {
    final scheme = ColorScheme.fromSeed(
      seedColor: accent,
      brightness: Brightness.dark,
    ).copyWith(
      primary: accent,
      secondary: accent2,
      surface: surface,
      surfaceContainerHighest: surface2,
      onSurface: textPrimary,
      onSurfaceVariant: textDim,
      error: danger,
    );

    final base = ThemeData(
      useMaterial3: true,
      colorScheme: scheme,
      scaffoldBackgroundColor: bg,
      fontFamily: fontFamily,
      splashFactory: InkSparkle.splashFactory,
    );

    final baseText = base.textTheme.apply(
      fontFamily: fontFamily,
      bodyColor: textPrimary,
      displayColor: textPrimary,
    );

    return base.copyWith(
      textTheme: baseText,
      appBarTheme: const AppBarTheme(
        backgroundColor: Colors.transparent,
        elevation: 0,
        centerTitle: false,
        titleTextStyle: TextStyle(
          color: textPrimary,
          fontSize: 17,
          fontWeight: FontWeight.w600,
          fontFamily: fontFamily,
        ),
        iconTheme: IconThemeData(color: textPrimary),
      ),
      cardTheme: CardThemeData(
        color: surface,
        elevation: 0,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(16),
          side: BorderSide(color: Colors.white.withValues(alpha: 0.06)),
        ),
        margin: EdgeInsets.zero,
      ),
      inputDecorationTheme: InputDecorationTheme(
        filled: true,
        fillColor: surface2,
        hintStyle: const TextStyle(color: textDim),
        border: OutlineInputBorder(
          borderRadius: BorderRadius.circular(14),
          borderSide: BorderSide.none,
        ),
        contentPadding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
      ),
      chipTheme: base.chipTheme.copyWith(
        backgroundColor: surface2,
        side: BorderSide.none,
        labelStyle: const TextStyle(color: textDim, fontSize: 12),
      ),
      dividerTheme: const DividerThemeData(
        color: Color(0x14FFFFFF),
        thickness: 1,
      ),
      listTileTheme: const ListTileThemeData(
        iconColor: textDim,
        textColor: textPrimary,
      ),
      snackBarTheme: SnackBarThemeData(
        backgroundColor: surface2,
        contentTextStyle: const TextStyle(color: textPrimary),
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
        behavior: SnackBarBehavior.floating,
      ),
    );
  }
}

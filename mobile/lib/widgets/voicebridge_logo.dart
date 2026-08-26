import 'package:flutter/material.dart';

/// Логотип VoiceBridge — повторяет иконку десктопа: тёмный градиент +
/// циановая «голосовая волна».
class VoiceBridgeLogo extends StatelessWidget {
  final double size;

  const VoiceBridgeLogo({super.key, this.size = 28});

  @override
  Widget build(BuildContext context) {
    return Center(
      child: SizedBox(
        width: size,
        height: size,
        child: CustomPaint(painter: _WaveLogoPainter()),
      ),
    );
  }
}

class _WaveLogoPainter extends CustomPainter {
  static const _bgTop = Color(0xFF1B2637);
  static const _bgBottom = Color(0xFF0A0F17);
  static const _barTop = Color(0xFF67E8F9);
  static const _barBottom = Color(0xFF0EA5E9);

  static const _amps = [0.40, 0.80, 0.55, 1.00, 0.55, 0.80, 0.40];

  @override
  void paint(Canvas canvas, Size size) {
    // Вписываем квадрат в центр — защита от растягивания не-квадратным parent.
    final side = size.shortestSide;
    final rect = Rect.fromCenter(
      center: Offset(size.width / 2, size.height / 2),
      width: side,
      height: side,
    );
    final s = rect.width;

    final bgPaint = Paint()
      ..shader = const LinearGradient(
        begin: Alignment.topCenter,
        end: Alignment.bottomCenter,
        colors: [_bgTop, _bgBottom],
      ).createShader(rect);
    canvas.drawRRect(
      RRect.fromRectAndRadius(rect, Radius.circular(s * 0.22)),
      bgPaint,
    );

    final barPaint = Paint()
      ..shader = const LinearGradient(
        begin: Alignment.topCenter,
        end: Alignment.bottomCenter,
        colors: [_barTop, _barBottom],
      ).createShader(rect);

    const n = 7; // _amps.length
    const areaFrac = 0.60; // столбцы занимают центральные 60% ширины
    final areaW = s * areaFrac;
    final gap = areaW * 0.06;
    final barW = (areaW - gap * (n - 1)) / n;
    final startX = rect.left + (s - areaW) / 2;
    final cy = rect.center.dy;
    final maxHalf = s * 0.28;

    for (var i = 0; i < n; i++) {
      final bh = maxHalf * _amps[i];
      final x = startX + i * (barW + gap);
      canvas.drawRRect(
        RRect.fromRectAndRadius(
          Rect.fromCenter(
            center: Offset(x + barW / 2, cy),
            width: barW,
            height: bh * 2,
          ),
          Radius.circular(barW / 2),
        ),
        barPaint,
      );
    }
  }

  @override
  bool shouldRepaint(covariant CustomPainter oldDelegate) => false;
}

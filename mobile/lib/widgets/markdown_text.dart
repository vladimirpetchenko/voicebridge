import 'package:flutter/material.dart';
import 'package:markdown_widget/markdown_widget.dart';

/// Рендерит markdown (GFM: код с подсветкой, таблицы, списки) внутри чата.
class MarkdownText extends StatelessWidget {
  final String data;

  const MarkdownText({super.key, required this.data});

  @override
  Widget build(BuildContext context) {
    final generator = MarkdownGenerator(
      linesMargin: const EdgeInsets.symmetric(vertical: 2),
    );
    final widgets =
        generator.buildWidgets(data, config: MarkdownConfig.darkConfig);
    return SelectionArea(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: widgets,
      ),
    );
  }
}

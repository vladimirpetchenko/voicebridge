import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../app_state.dart';
import '../models.dart';
import '../theme.dart';
import 'markdown_text.dart';

/// Пузырь сообщения (пользователь справа, ассистент слева).
class MessageBubble extends StatelessWidget {
  final ConversationMessage message;
  final bool streaming;

  const MessageBubble({super.key, required this.message, this.streaming = false});

  @override
  Widget build(BuildContext context) {
    final isAssistant = message.isAssistant;
    return Align(
      alignment: isAssistant ? Alignment.centerLeft : Alignment.centerRight,
      child: Container(
        margin: const EdgeInsets.symmetric(vertical: 6),
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
        constraints: BoxConstraints(
          maxWidth: MediaQuery.of(context).size.width * 0.82,
        ),
        decoration: BoxDecoration(
          color: isAssistant ? AppTheme.surface2 : const Color(0xFF2A3A66),
          borderRadius: BorderRadius.only(
            topLeft: const Radius.circular(16),
            topRight: const Radius.circular(16),
            bottomLeft: Radius.circular(isAssistant ? 4 : 16),
            bottomRight: Radius.circular(isAssistant ? 16 : 4),
          ),
        ),
        child: isAssistant
            ? Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                mainAxisSize: MainAxisSize.min,
                children: [
                  if (message.reasoning.isNotEmpty)
                    ReasoningBlock(
                      text: message.reasoning,
                      streaming: streaming,
                    ),
                  if (message.text.isNotEmpty) MarkdownText(data: message.text),
                ],
              )
            : SelectableText(
                message.text,
                style: const TextStyle(
                  color: Colors.white,
                  fontSize: 15,
                  height: 1.4,
                ),
              ),
      ),
    );
  }
}

/// Сворачиваемый блок размышлений (reasoning).
class ReasoningBlock extends StatefulWidget {
  final String text;
  final bool streaming;

  const ReasoningBlock({super.key, required this.text, required this.streaming});

  @override
  State<ReasoningBlock> createState() => _ReasoningBlockState();
}

class _ReasoningBlockState extends State<ReasoningBlock> {
  bool _open = false;

  @override
  void initState() {
    super.initState();
    _open = widget.streaming;
  }

  @override
  void didUpdateWidget(covariant ReasoningBlock old) {
    super.didUpdateWidget(old);
    if (old.streaming && !widget.streaming) {
      _open = false;
    }
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        InkWell(
          onTap: () => setState(() => _open = !_open),
          borderRadius: BorderRadius.circular(8),
          child: Padding(
            padding: const EdgeInsets.symmetric(vertical: 4),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Icon(
                  _open ? Icons.expand_more_rounded : Icons.chevron_right_rounded,
                  size: 16,
                  color: AppTheme.textDim,
                ),
                const SizedBox(width: 4),
                Text(
                  widget.streaming ? 'Размышляет…' : 'Размышление',
                  style: const TextStyle(
                    color: AppTheme.textDim,
                    fontSize: 12,
                    fontWeight: FontWeight.w600,
                  ),
                ),
              ],
            ),
          ),
        ),
        if (_open)
          Container(
            margin: const EdgeInsets.only(top: 2, bottom: 8),
            padding: const EdgeInsets.only(left: 12),
            decoration: const BoxDecoration(
              border: Border(
                left: BorderSide(color: AppTheme.accent, width: 2),
              ),
            ),
            child: MarkdownText(data: widget.text),
          ),
      ],
    );
  }
}

/// Индикатор «OpenCode думает…».
class ThinkingIndicator extends StatelessWidget {
  const ThinkingIndicator({super.key});

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 10),
      child: Row(
        children: [
          const SizedBox(
            width: 14,
            height: 14,
            child: CircularProgressIndicator(strokeWidth: 2),
          ),
          const SizedBox(width: 10),
          Text(
            'OpenCode думает…',
            style: TextStyle(color: AppTheme.textDim, fontSize: 13),
          ),
        ],
      ),
    );
  }
}

/// Карточка запроса разрешения от OpenCode.
class PermissionCard extends StatelessWidget {
  final PermissionRequest request;

  const PermissionCard({super.key, required this.request});

  @override
  Widget build(BuildContext context) {
    return Card(
      margin: const EdgeInsets.symmetric(vertical: 6),
      child: Padding(
        padding: const EdgeInsets.all(14),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const Text(
              'OpenCode запрашивает разрешение',
              style: TextStyle(fontWeight: FontWeight.w600),
            ),
            const SizedBox(height: 4),
            Text(
              'Инструмент: ${request.permission.isEmpty ? '?' : request.permission}',
              style: const TextStyle(color: AppTheme.textDim, fontSize: 13),
            ),
            if (request.patterns.isNotEmpty)
              Padding(
                padding: const EdgeInsets.only(top: 6),
                child: Text(
                  request.patterns.join('\n'),
                  style: const TextStyle(color: AppTheme.textDim, fontSize: 12),
                ),
              ),
            const SizedBox(height: 10),
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: [
                FilledButton(
                  onPressed: () => context
                      .read<AppController>()
                      .replyPermission(request, 'once'),
                  child: const Text('Разрешить'),
                ),
                OutlinedButton(
                  onPressed: () => context
                      .read<AppController>()
                      .replyPermission(request, 'always'),
                  child: const Text('Всегда'),
                ),
                OutlinedButton(
                  onPressed: () => context
                      .read<AppController>()
                      .replyPermission(request, 'reject'),
                  style: OutlinedButton.styleFrom(
                    foregroundColor: const Color(0xFFFF6B6B),
                  ),
                  child: const Text('Запретить'),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}

/// Карточка вопроса OpenCode (с вариантами ответа).
class QuestionCard extends StatelessWidget {
  final QuestionRequest request;

  const QuestionCard({super.key, required this.request});

  @override
  Widget build(BuildContext context) {
    final first = request.questions.isNotEmpty ? request.questions.first : null;
    return Card(
      margin: const EdgeInsets.symmetric(vertical: 6),
      child: Padding(
        padding: const EdgeInsets.all(14),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              first?.header ?? 'Вопрос OpenCode',
              style: const TextStyle(fontWeight: FontWeight.w600),
            ),
            if (first != null && first.question.isNotEmpty) ...[
              const SizedBox(height: 4),
              Text(
                first.question,
                style: const TextStyle(color: AppTheme.textDim, fontSize: 13),
              ),
            ],
            if (first != null && first.options.isNotEmpty) ...[
              const SizedBox(height: 10),
              Wrap(
                spacing: 8,
                runSpacing: 8,
                children: [
                  for (final opt in first.options)
                    OutlinedButton(
                      onPressed: () => context.read<AppController>().answerQuestion(
                            request,
                            [
                              [opt.label]
                            ],
                          ),
                      child: Text(opt.label),
                    ),
                ],
              ),
            ],
            const SizedBox(height: 6),
            TextButton(
              onPressed: () =>
                  context.read<AppController>().rejectQuestion(request),
              style: TextButton.styleFrom(
                foregroundColor: const Color(0xFFFF6B6B),
              ),
              child: const Text('Отклонить'),
            ),
          ],
        ),
      ),
    );
  }
}

/// Чипы запущенных инструментов OpenCode.
class ToolChips extends StatelessWidget {
  final List<ToolAction> tools;

  const ToolChips({super.key, required this.tools});

  @override
  Widget build(BuildContext context) {
    return Wrap(
      spacing: 6,
      runSpacing: 6,
      children: [
        for (final tool in tools)
          Chip(
            visualDensity: VisualDensity.compact,
            avatar: Icon(
              switch (tool.state) {
                'running' => Icons.autorenew,
                'done' => Icons.check,
                'failed' => Icons.close,
                _ => Icons.build,
              },
              size: 16,
              color: tool.state == 'failed'
                  ? const Color(0xFFFF6B6B)
                  : AppTheme.textDim,
            ),
            label: Text(tool.name),
          ),
      ],
    );
  }
}

/// Строка токенов/стоимости сессии.
class UsageBar extends StatelessWidget {
  final SessionUsage usage;

  const UsageBar({super.key, required this.usage});

  @override
  Widget build(BuildContext context) {
    final parts = <String>[
      '${usage.tokensTotal} токенов',
      if (usage.model.isNotEmpty) usage.model,
      if (usage.cost > 0) '\$${usage.cost.toStringAsFixed(4)}',
    ];
    return Container(
      width: double.infinity,
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 6),
      decoration: const BoxDecoration(
        color: AppTheme.surface,
        border: Border(top: BorderSide(color: Color(0x14FFFFFF))),
      ),
      child: Text(
        parts.join(' · '),
        style: const TextStyle(color: AppTheme.textDim, fontSize: 12),
      ),
    );
  }
}

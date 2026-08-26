import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../app_state.dart';
import '../models.dart';

/// Экран чата с выбранной сессией: сообщения, стрим, инструменты, строка ввода.
class ChatScreen extends StatefulWidget {
  const ChatScreen({super.key});

  @override
  State<ChatScreen> createState() => _ChatScreenState();
}

class _ChatScreenState extends State<ChatScreen> {
  final _inputController = TextEditingController();
  final _scrollController = ScrollController();

  @override
  void dispose() {
    _inputController.dispose();
    _scrollController.dispose();
    super.dispose();
  }

  void _send() {
    final text = _inputController.text.trim();
    if (text.isEmpty) return;
    _inputController.clear();
    context.read<AppController>().sendPrompt(text);
  }

  @override
  Widget build(BuildContext context) {
    final controller = context.watch<AppController>();
    final title = controller.selectedSession?.title ??
        controller.selectedSessionId ??
        'Чат';

    return Scaffold(
      appBar: AppBar(
        title: Text(title, overflow: TextOverflow.ellipsis),
        actions: [
          if (controller.busy)
            IconButton(
              tooltip: 'Остановить',
              icon: const Icon(Icons.stop_circle_outlined),
              onPressed: () => context.read<AppController>().abort(),
            ),
        ],
      ),
      body: Column(
        children: [
          Expanded(
            child: ListView(
              controller: _scrollController,
              padding: const EdgeInsets.all(12),
              children: [
                for (int i = 0; i < controller.messages.length; i++)
                  _MessageBubble(message: controller.messages[i]),
                for (final p in controller.pendingPermissions)
                  _PermissionCard(request: p),
                for (final q in controller.pendingQuestions)
                  _QuestionCard(request: q),
                if (controller.busy && controller.messages.isNotEmpty)
                  const Padding(
                    padding: EdgeInsets.symmetric(vertical: 8),
                    child: Row(
                      children: [
                        SizedBox(
                          width: 12,
                          height: 12,
                          child: CircularProgressIndicator(strokeWidth: 2),
                        ),
                        SizedBox(width: 8),
                        Text('думает…', style: TextStyle(color: Colors.grey)),
                      ],
                    ),
                  ),
                if (controller.tools.isNotEmpty) _ToolChips(tools: controller.tools),
              ],
            ),
          ),
          if (controller.usage != null)
            _UsageBar(usage: controller.usage!),
          SafeArea(
            child: Padding(
              padding: const EdgeInsets.fromLTRB(12, 4, 12, 12),
              child: Row(
                children: [
                  Expanded(
                    child: TextField(
                      controller: _inputController,
                      minLines: 1,
                      maxLines: 5,
                      textInputAction: TextInputAction.send,
                      onSubmitted: (_) => _send(),
                      decoration: const InputDecoration(
                        hintText: 'Сообщение…',
                        border: OutlineInputBorder(),
                        isDense: true,
                      ),
                    ),
                  ),
                  const SizedBox(width: 8),
                  IconButton.filled(
                    tooltip: 'Отправить',
                    icon: const Icon(Icons.send),
                    onPressed: _send,
                  ),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _MessageBubble extends StatelessWidget {
  final ConversationMessage message;

  const _MessageBubble({required this.message});

  @override
  Widget build(BuildContext context) {
    final isAssistant = message.isAssistant;
    final scheme = Theme.of(context).colorScheme;
    return Align(
      alignment: isAssistant ? Alignment.centerLeft : Alignment.centerRight,
      child: Container(
        margin: const EdgeInsets.symmetric(vertical: 4),
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
        constraints: const BoxConstraints(maxWidth: 320),
        decoration: BoxDecoration(
          color: isAssistant ? scheme.surfaceContainerHighest : scheme.primary,
          borderRadius: BorderRadius.circular(12),
        ),
        child: Text(
          message.text.isEmpty ? ' ' : message.text,
          style: TextStyle(
            color: isAssistant ? scheme.onSurface : scheme.onPrimary,
          ),
        ),
      ),
    );
  }
}

class _ToolChips extends StatelessWidget {
  final List<ToolAction> tools;

  const _ToolChips({required this.tools});

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
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
            ),
            label: Text(tool.name),
            backgroundColor: tool.state == 'failed'
                ? scheme.errorContainer
                : scheme.surfaceContainerHighest,
          ),
      ],
    );
  }
}

class _PermissionCard extends StatelessWidget {
  final PermissionRequest request;

  const _PermissionCard({required this.request});

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Card(
      margin: const EdgeInsets.symmetric(vertical: 6),
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text('OpenCode запрашивает разрешение',
                style: Theme.of(context).textTheme.titleSmall),
            const SizedBox(height: 4),
            Text('Инструмент: ${request.permission.isEmpty ? '?' : request.permission}'),
            if (request.patterns.isNotEmpty)
              Text(
                request.patterns.join('\n'),
                style: TextStyle(color: scheme.onSurfaceVariant, fontSize: 12),
              ),
            const SizedBox(height: 8),
            Wrap(
              spacing: 8,
              children: [
                FilledButton(
                  onPressed: () =>
                      context.read<AppController>().replyPermission(request, 'once'),
                  child: const Text('Разрешить'),
                ),
                OutlinedButton(
                  onPressed: () =>
                      context.read<AppController>().replyPermission(request, 'always'),
                  child: const Text('Всегда'),
                ),
                OutlinedButton(
                  onPressed: () =>
                      context.read<AppController>().replyPermission(request, 'reject'),
                  style: OutlinedButton.styleFrom(
                    foregroundColor: scheme.error,
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

class _QuestionCard extends StatelessWidget {
  final QuestionRequest request;

  const _QuestionCard({required this.request});

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final first = request.questions.isNotEmpty ? request.questions.first : null;
    return Card(
      margin: const EdgeInsets.symmetric(vertical: 6),
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(first?.header ?? 'Вопрос OpenCode',
                style: Theme.of(context).textTheme.titleSmall),
            if (first != null && first.question.isNotEmpty) ...[
              const SizedBox(height: 4),
              Text(first.question),
            ],
            if (first != null && first.options.isNotEmpty) ...[
              const SizedBox(height: 8),
              Wrap(
                spacing: 8,
                runSpacing: 8,
                children: [
                  for (final opt in first.options)
                    OutlinedButton(
                      onPressed: () => context
                          .read<AppController>()
                          .answerQuestion(request, [
                        [opt.label]
                      ]),
                      child: Text(opt.label),
                    ),
                ],
              ),
            ],
            const SizedBox(height: 8),
            TextButton(
              onPressed: () =>
                  context.read<AppController>().rejectQuestion(request),
              style: TextButton.styleFrom(foregroundColor: scheme.error),
              child: const Text('Отклонить'),
            ),
          ],
        ),
      ),
    );
  }
}

class _UsageBar extends StatelessWidget {
  final SessionUsage usage;

  const _UsageBar({required this.usage});

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    String cost() {
      if (usage.cost == 0) return '';
      return ' · \$${usage.cost.toStringAsFixed(4)}';
    }

    return Container(
      width: double.infinity,
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
      color: scheme.surfaceContainerHighest,
      child: Text(
        '${usage.tokensTotal} токенов'
        '${usage.model.isNotEmpty ? ' · ${usage.model}' : ''}'
        '${cost()}',
        style: TextStyle(color: scheme.onSurfaceVariant, fontSize: 12),
      ),
    );
  }
}

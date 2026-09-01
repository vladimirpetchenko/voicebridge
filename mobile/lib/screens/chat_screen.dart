import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../app_state.dart';
import '../theme.dart';
import '../widgets/chat_widgets.dart';
import 'git_screen.dart';

/// Экран чата с выбранной сессией: markdown, стрим, инструменты, действия.
class ChatScreen extends StatefulWidget {
  const ChatScreen({super.key});

  @override
  State<ChatScreen> createState() => _ChatScreenState();
}

class _ChatScreenState extends State<ChatScreen> {
  final _inputController = TextEditingController();

  @override
  void dispose() {
    _inputController.dispose();
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
    final project = controller.selectedInstance?.name;

    return Scaffold(
      appBar: AppBar(
        title: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(title, overflow: TextOverflow.ellipsis),
            if (project != null && project.isNotEmpty)
              Text(
                project,
                overflow: TextOverflow.ellipsis,
                style: const TextStyle(fontSize: 12, color: AppTheme.textDim),
              ),
          ],
        ),
        actions: [
          IconButton(
            tooltip: 'Изменения',
            icon: const Icon(Icons.account_tree_outlined),
            onPressed: () {
              Navigator.of(context).push(
                MaterialPageRoute(builder: (_) => const GitScreen()),
              );
            },
          ),
          if (controller.busy)
            IconButton(
              tooltip: 'Остановить',
              icon: const Icon(Icons.stop_circle_outlined, color: Color(0xFFFF6B6B)),
              onPressed: () => context.read<AppController>().abort(),
            ),
        ],
      ),
      body: Column(
        children: [
          Expanded(
            child: ListView(
              reverse: true,
              padding: const EdgeInsets.fromLTRB(16, 8, 16, 8),
              children: _buildItems(controller).reversed.toList(),
            ),
          ),
          if (controller.usage != null) UsageBar(usage: controller.usage!),
          SafeArea(
            top: false,
            child: Padding(
              padding: const EdgeInsets.fromLTRB(12, 8, 12, 12),
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.end,
                children: [
                  Expanded(
                    child: TextField(
                      controller: _inputController,
                      minLines: 1,
                      maxLines: 6,
                      textInputAction: TextInputAction.newline,
                      onSubmitted: (_) => _send(),
                      decoration: const InputDecoration(
                        hintText: 'Сообщение…',
                      ),
                    ),
                  ),
                  const SizedBox(width: 8),
                  IconButton.filled(
                    tooltip: 'Отправить',
                    onPressed: _send,
                    style: IconButton.styleFrom(
                      backgroundColor: AppTheme.accent,
                      foregroundColor: Colors.white,
                    ),
                    icon: const Icon(Icons.arrow_upward_rounded),
                  ),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }

  List<Widget> _buildItems(AppController controller) {
    final items = <Widget>[];

    for (final msg in controller.messages) {
      if (msg.isAssistant && msg.text.isEmpty && msg.reasoning.isEmpty) continue;
      items.add(MessageBubble(
        message: msg,
        streaming: controller.busy && msg.text.isEmpty,
      ));
    }
    for (final p in controller.pendingPermissions) {
      items.add(PermissionCard(request: p));
    }
    for (final q in controller.pendingQuestions) {
      items.add(QuestionCard(request: q));
    }
    if (controller.busy) {
      items.add(const ThinkingIndicator());
    }
    if (controller.tools.isNotEmpty) {
      items.add(ToolChips(tools: controller.tools));
    }
    return items;
  }
}

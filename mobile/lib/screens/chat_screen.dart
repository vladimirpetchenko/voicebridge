import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../app_state.dart';
import '../models.dart';
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
  final _scrollController = ScrollController();
  bool _atBottom = true;

  @override
  void initState() {
    super.initState();
    _scrollController.addListener(_onScroll);
  }

  @override
  void dispose() {
    _scrollController.removeListener(_onScroll);
    _scrollController.dispose();
    _inputController.dispose();
    super.dispose();
  }

  void _onScroll() {
    if (!_scrollController.hasClients) return;
    // В reverse-списке offset 0 — низ. Считаем «внизу», если рядом с низом.
    final pixels = _scrollController.position.pixels;
    final at = pixels <= 40;
    if (at != _atBottom) {
      setState(() => _atBottom = at);
    }
  }

  void _scrollToBottom() {
    if (!_scrollController.hasClients) return;
    _scrollController.animateTo(
      0,
      duration: const Duration(milliseconds: 250),
      curve: Curves.easeOut,
    );
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

    // Если пользователь у низа — докручиваем к последнему сообщению; если ушёл
    // вверх читать историю — не трогаем позицию.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || !_atBottom || !_scrollController.hasClients) return;
      _scrollController.jumpTo(0);
    });

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
            child: Stack(
              children: [
                ListView(
                  controller: _scrollController,
                  reverse: true,
                  padding: const EdgeInsets.fromLTRB(16, 8, 16, 8),
                  children: _buildItems(controller).reversed.toList(),
                ),
                if (!_atBottom)
                  Positioned(
                    right: 16,
                    bottom: 16,
                    child: Material(
                      color: AppTheme.surface2,
                      shape: const CircleBorder(
                        side: BorderSide(color: Color(0x22FFFFFF)),
                      ),
                      child: IconButton(
                        tooltip: 'К последнему сообщению',
                        icon: const Icon(
                          Icons.arrow_downward_rounded,
                          color: AppTheme.textPrimary,
                        ),
                        onPressed: _scrollToBottom,
                      ),
                    ),
                  ),
              ],
            ),
          ),
          if (controller.pendingPermissions.isNotEmpty ||
              controller.pendingQuestions.isNotEmpty)
            _ActionDock(
              permissions: controller.pendingPermissions,
              questions: controller.pendingQuestions,
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
    if (controller.busy) {
      items.add(const ThinkingIndicator());
    }
    if (controller.tools.isNotEmpty) {
      items.add(ToolChips(tools: controller.tools));
    }
    return items;
  }
}

/// Док запросов действий (разрешения/вопросы) — над полем ввода, всегда на виду.
class _ActionDock extends StatelessWidget {
  final List<PermissionRequest> permissions;
  final List<QuestionRequest> questions;

  const _ActionDock({required this.permissions, required this.questions});

  @override
  Widget build(BuildContext context) {
    return Container(
      constraints: BoxConstraints(
        maxHeight: MediaQuery.of(context).size.height * 0.45,
      ),
      decoration: const BoxDecoration(
        color: AppTheme.surface,
        border: Border(top: BorderSide(color: Color(0x14FFFFFF))),
      ),
      child: SingleChildScrollView(
        padding: const EdgeInsets.fromLTRB(12, 10, 12, 10),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            for (final p in permissions) PermissionCard(request: p),
            for (final q in questions) QuestionCard(request: q),
          ],
        ),
      ),
    );
  }
}

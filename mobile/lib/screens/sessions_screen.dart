import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../app_state.dart';
import '../models.dart';
import 'chat_screen.dart';

/// Лаунчер: список экземпляров OpenCode и их сессий. Выбор сессии открывает
/// чат.
class SessionsScreen extends StatelessWidget {
  const SessionsScreen({super.key});

  Future<void> _open(BuildContext context, OpenCodeInstance instance,
      OpenCodeSession session) async {
    final controller = context.read<AppController>();
    await controller.selectSession(instance, session);
    if (!context.mounted) return;
    await Navigator.of(context).push(
      MaterialPageRoute(builder: (_) => const ChatScreen()),
    );
    // После возврата из чата обновляем список (сессии могли измениться).
    if (context.mounted) {
      controller.refreshSessions();
    }
  }

  @override
  Widget build(BuildContext context) {
    final controller = context.watch<AppController>();
    final instances = controller.instances;

    return Scaffold(
      appBar: AppBar(
        title: const Text('Сессии'),
        actions: [
          IconButton(
            tooltip: 'Обновить',
            icon: const Icon(Icons.refresh),
            onPressed: () => context.read<AppController>().refreshSessions(),
          ),
          IconButton(
            tooltip: 'Отключиться',
            icon: const Icon(Icons.link_off),
            onPressed: () => context.read<AppController>().disconnect(),
          ),
        ],
      ),
      body: instances.isEmpty
          ? const Center(child: Text('Нет запущенных серверов OpenCode'))
          : RefreshIndicator(
              onRefresh: () => context.read<AppController>().refreshSessions(),
              child: ListView(
                children: [
                  for (final instance in instances)
                    _InstanceSection(
                      instance: instance,
                      onSelect: (s) => _open(context, instance, s),
                    ),
                ],
              ),
            ),
    );
  }
}

class _InstanceSection extends StatelessWidget {
  final OpenCodeInstance instance;
  final void Function(OpenCodeSession) onSelect;

  const _InstanceSection({required this.instance, required this.onSelect});

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 16, 16, 8),
          child: Text(
            '${instance.name}  ·  :${instance.port}',
            style: Theme.of(context).textTheme.titleSmall,
          ),
        ),
        if (instance.sessions.isEmpty)
          const Padding(
            padding: EdgeInsets.symmetric(horizontal: 16),
            child: Text('Нет сессий', style: TextStyle(color: Colors.grey)),
          ),
        for (final session in instance.sessions)
          ListTile(
            leading: const Icon(Icons.chat_bubble_outline),
            title: Text(session.title.isEmpty ? session.id : session.title),
            subtitle: Text(session.directory),
            trailing: const Icon(Icons.chevron_right),
            onTap: () => onSelect(session),
          ),
        const Divider(height: 1),
      ],
    );
  }
}

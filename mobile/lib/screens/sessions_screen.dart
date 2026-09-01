import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../app_state.dart';
import '../models.dart';
import '../widgets/project_card.dart';
import '../widgets/voicebridge_logo.dart';
import 'chat_screen.dart';

/// Лаунчер: проекты (запуск/остановка) и сессии по ним. Выбор сессии открывает
/// чат.
class SessionsScreen extends StatelessWidget {
  const SessionsScreen({super.key});

  Map<int, List<OpenCodeSession>> _sessionsByPort(List<OpenCodeInstance> instances) {
    final map = <int, List<OpenCodeSession>>{};
    for (final inst in instances) {
      map[inst.port] = inst.sessions;
    }
    return map;
  }

  String _relTime(int ms) {
    if (ms <= 0) return '';
    final diff = DateTime.now().millisecondsSinceEpoch - ms;
    if (diff < 60000) return 'только что';
    final m = diff ~/ 60000;
    if (m < 60) return '$m мин назад';
    final h = m ~/ 60;
    if (h < 24) return '$h ч назад';
    return '${h ~/ 24} дн назад';
  }

  Future<void> _open(BuildContext context, OpenCodeInstance instance,
      OpenCodeSession session) async {
    final controller = context.read<AppController>();
    await controller.selectSession(instance, session);
    if (!context.mounted) return;
    await Navigator.of(context).push(
      MaterialPageRoute(builder: (_) => const ChatScreen()),
    );
    if (context.mounted) {
      controller.refreshSessions();
      controller.refreshProjects();
    }
  }

  Future<void> _createAndOpen(BuildContext context, Project project) async {
    final controller = context.read<AppController>();
    await controller.createSession(project.port, project.worktree);
    if (!context.mounted) return;
    await Navigator.of(context).push(
      MaterialPageRoute(builder: (_) => const ChatScreen()),
    );
    if (context.mounted) {
      controller.refreshSessions();
      controller.refreshProjects();
    }
  }

  @override
  Widget build(BuildContext context) {
    final controller = context.watch<AppController>();
    final byPort = _sessionsByPort(controller.instances);
    final instanceByPort = {
      for (final i in controller.instances) i.port: i,
    };
    final projectPorts = controller.projects.map((p) => p.port).toSet();
    final hiddenSet = controller.hiddenProjects.toSet();

    final visibleProjects =
        controller.projects.where((p) => !hiddenSet.contains(p.worktree)).toList();
    final orphans = controller.instances
        .where((i) => !projectPorts.contains(i.port) && !hiddenSet.contains(i.id))
        .toList();

    return Scaffold(
      appBar: AppBar(
        title: const Row(
          children: [
            VoiceBridgeLogo(size: 24),
            SizedBox(width: 10),
            Text('VoiceBridge'),
          ],
        ),
        actions: [
          IconButton(
            tooltip: 'Обновить',
            icon: const Icon(Icons.refresh_rounded),
            onPressed: () {
              context.read<AppController>().refreshSessions();
              context.read<AppController>().refreshProjects();
              context.read<AppController>().refreshHidden();
            },
          ),
          IconButton(
            tooltip: 'Отключиться',
            icon: const Icon(Icons.link_off_rounded),
            onPressed: () => context.read<AppController>().disconnect(),
          ),
        ],
      ),
      body: RefreshIndicator(
        onRefresh: () async {
          final c = context.read<AppController>();
          await c.refreshSessions();
          await c.refreshProjects();
          await c.refreshHidden();
        },
        child: ListView(
          physics: const AlwaysScrollableScrollPhysics(),
          padding: const EdgeInsets.fromLTRB(16, 8, 16, 24),
          children: [
            SectionHeader(title: 'Проекты', count: visibleProjects.length),
            if (visibleProjects.isEmpty && orphans.isEmpty)
              const EmptyCard(
                icon: Icons.folder_open_rounded,
                text: 'Проекты не найдены. Откройте opencode в папке проекта.',
              ),
            for (final project in visibleProjects)
              ProjectCard(
                project: project,
                sessions: byPort[project.port] ?? const [],
                relTime: _relTime,
                onStart: () => context
                    .read<AppController>()
                    .startProject(project.worktree),
                onStop: () => context
                    .read<AppController>()
                    .stopProject(project.worktree),
                onCreateSession: () => _createAndOpen(context, project),
                onHide: () => context
                    .read<AppController>()
                    .hideProject(project.worktree),
                onOpenSession: (session) {
                  final instance = instanceByPort[project.port];
                  if (instance != null) {
                    _open(context, instance, session);
                  }
                },
              ),
            if (orphans.isNotEmpty) ...[
              const SizedBox(height: 16),
              SectionHeader(title: 'Другие серверы', count: orphans.length),
              for (final inst in orphans)
                ProjectCard(
                  project: Project(
                    id: inst.id,
                    worktree: inst.id,
                    name: inst.name.isEmpty ? 'сервер' : inst.name,
                    updated: 0,
                    running: true,
                    port: inst.port,
                  ),
                  sessions: inst.sessions,
                  relTime: _relTime,
                  onStart: () {},
                  onStop: () {},
                  onCreateSession: () async {
                    final c = context.read<AppController>();
                    await c.createSession(inst.port, inst.id);
                    if (!context.mounted) return;
                    await Navigator.of(context)
                        .push(MaterialPageRoute(builder: (_) => const ChatScreen()));
                  },
                  onHide: () => context.read<AppController>().hideProject(inst.id),
                  onOpenSession: (session) => _open(context, inst, session),
                ),
            ],
            if (controller.hiddenProjects.isNotEmpty) ...[
              const SizedBox(height: 16),
              SectionHeader(
                title: 'Скрытые проекты',
                count: controller.hiddenProjects.length,
              ),
              for (final w in controller.hiddenProjects)
                HiddenProjectRow(
                  name: () {
                    final matches =
                        controller.projects.where((p) => p.worktree == w);
                    return matches.isEmpty ? w : matches.first.name;
                  }(),
                  onRestore: () => context
                      .read<AppController>()
                      .unhideProject(w),
                ),
            ],
          ],
        ),
      ),
    );
  }
}

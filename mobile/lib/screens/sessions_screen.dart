import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../app_state.dart';
import '../models.dart';
import '../theme.dart';
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
            _SectionHeader(title: 'Проекты', count: visibleProjects.length),
            if (visibleProjects.isEmpty && orphans.isEmpty)
              const _EmptyCard(
                icon: Icons.folder_open_rounded,
                text: 'Проекты не найдены. Откройте opencode в папке проекта.',
              ),
            for (final project in visibleProjects)
              _ProjectCard(
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
              _SectionHeader(title: 'Другие серверы', count: orphans.length),
              for (final inst in orphans)
                _ProjectCard(
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
              _SectionHeader(
                title: 'Скрытые проекты',
                count: controller.hiddenProjects.length,
              ),
              for (final w in controller.hiddenProjects)
                _HiddenProjectRow(
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

class _SectionHeader extends StatelessWidget {
  final String title;
  final int count;

  const _SectionHeader({required this.title, required this.count});

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(4, 12, 4, 10),
      child: Row(
        children: [
          Text(
            title,
            style: const TextStyle(
              fontSize: 13,
              fontWeight: FontWeight.w700,
              letterSpacing: 0.4,
              color: AppTheme.textDim,
            ),
          ),
          const SizedBox(width: 8),
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 7, vertical: 1),
            decoration: BoxDecoration(
              color: AppTheme.surface2,
              borderRadius: BorderRadius.circular(99),
            ),
            child: Text(
              '$count',
              style: const TextStyle(fontSize: 11, color: AppTheme.textDim),
            ),
          ),
        ],
      ),
    );
  }
}

class _EmptyCard extends StatelessWidget {
  final IconData icon;
  final String text;

  const _EmptyCard({required this.icon, required this.text});

  @override
  Widget build(BuildContext context) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(20),
        child: Column(
          children: [
            Icon(icon, size: 28, color: AppTheme.textDim),
            const SizedBox(height: 10),
            Text(
              text,
              textAlign: TextAlign.center,
              style: const TextStyle(color: AppTheme.textDim, fontSize: 13),
            ),
          ],
        ),
      ),
    );
  }
}

class _HiddenProjectRow extends StatelessWidget {
  final String name;
  final VoidCallback onRestore;

  const _HiddenProjectRow({required this.name, required this.onRestore});

  @override
  Widget build(BuildContext context) {
    return Card(
      margin: const EdgeInsets.only(bottom: 10),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 8),
        child: Row(
          children: [
            const Icon(Icons.visibility_off_outlined,
                size: 16, color: AppTheme.textDim),
            const SizedBox(width: 10),
            Expanded(
              child: Text(
                name,
                overflow: TextOverflow.ellipsis,
                style: const TextStyle(fontSize: 14),
              ),
            ),
            TextButton.icon(
              onPressed: onRestore,
              icon: const Icon(Icons.rotate_left_rounded, size: 16),
              label: const Text('Вернуть'),
            ),
          ],
        ),
      ),
    );
  }
}

class _ProjectCard extends StatefulWidget {
  final Project project;
  final List<OpenCodeSession> sessions;
  final String Function(int) relTime;
  final VoidCallback onStart;
  final VoidCallback onStop;
  final VoidCallback onCreateSession;
  final VoidCallback onHide;
  final void Function(OpenCodeSession) onOpenSession;

  const _ProjectCard({
    required this.project,
    required this.sessions,
    required this.relTime,
    required this.onStart,
    required this.onStop,
    required this.onCreateSession,
    required this.onHide,
    required this.onOpenSession,
  });

  @override
  State<_ProjectCard> createState() => _ProjectCardState();
}

class _ProjectCardState extends State<_ProjectCard> {
  bool _expanded = false;

  @override
  Widget build(BuildContext context) {
    final project = widget.project;
    final sessions = widget.sessions;
    final visible = _expanded ? sessions : sessions.take(3).toList();
    final hidden = sessions.length - visible.length;

    return Card(
      margin: const EdgeInsets.only(bottom: 12),
      child: Padding(
        padding: const EdgeInsets.all(14),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Container(
                  width: 40,
                  height: 40,
                  decoration: BoxDecoration(
                    color: AppTheme.surface2,
                    borderRadius: BorderRadius.circular(12),
                  ),
                  child: const Icon(
                    Icons.folder_rounded,
                    color: AppTheme.accent,
                    size: 22,
                  ),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Row(
                        children: [
                          Flexible(
                            child: Text(
                              project.name,
                              overflow: TextOverflow.ellipsis,
                              style: const TextStyle(
                                fontSize: 15,
                                fontWeight: FontWeight.w600,
                              ),
                            ),
                          ),
                          if (project.running) ...[
                            const SizedBox(width: 6),
                            Container(
                              width: 7,
                              height: 7,
                              decoration: const BoxDecoration(
                                color: AppTheme.accent2,
                                shape: BoxShape.circle,
                              ),
                            ),
                          ],
                        ],
                      ),
                      const SizedBox(height: 2),
                      Text(
                        project.worktree,
                        overflow: TextOverflow.ellipsis,
                        style: const TextStyle(
                          fontSize: 12,
                          color: AppTheme.textDim,
                        ),
                      ),
                    ],
                  ),
                ),
                if (project.running)
                  IconButton(
                    tooltip: 'Остановить',
                    onPressed: widget.onStop,
                    icon: const Icon(Icons.stop_circle_outlined,
                        color: Color(0xFFFF6B6B)),
                  )
                else
                  IconButton(
                    tooltip: 'Запустить',
                    onPressed: widget.onStart,
                    icon: const Icon(Icons.play_circle_outline_rounded,
                        color: AppTheme.accent2),
                  ),
                IconButton(
                  tooltip: 'Скрыть проект',
                  onPressed: widget.onHide,
                  icon: const Icon(Icons.close_rounded,
                      size: 18, color: AppTheme.textDim),
                ),
              ],
            ),
            if (project.running) ...[
              const Divider(height: 20),
              if (sessions.isEmpty)
                const Text(
                  'Нет сессий',
                  style: TextStyle(color: AppTheme.textDim, fontSize: 13),
                )
              else
                for (final session in visible)
                  _SessionTile(
                    title: session.title.isEmpty ? session.id : session.title,
                    subtitle: widget.relTime(session.updatedAt),
                    onTap: () => widget.onOpenSession(session),
                  ),
              if (hidden > 0)
                InkWell(
                  onTap: () => setState(() => _expanded = !_expanded),
                  borderRadius: BorderRadius.circular(10),
                  child: Padding(
                    padding: const EdgeInsets.symmetric(vertical: 8),
                    child: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Icon(
                          _expanded
                              ? Icons.expand_less_rounded
                              : Icons.expand_more_rounded,
                          size: 16,
                          color: AppTheme.textDim,
                        ),
                        const SizedBox(width: 4),
                        Text(
                          _expanded ? 'Свернуть' : 'Ещё $hidden',
                          style: const TextStyle(
                            fontSize: 12,
                            color: AppTheme.textDim,
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
              const SizedBox(height: 8),
              SizedBox(
                width: double.infinity,
                child: OutlinedButton.icon(
                  onPressed: widget.onCreateSession,
                  icon: const Icon(Icons.add_rounded, size: 16),
                  label: const Text('Новая сессия'),
                  style: OutlinedButton.styleFrom(
                    foregroundColor: AppTheme.accent,
                    side: BorderSide(
                      color: AppTheme.accent.withValues(alpha: 0.4),
                    ),
                    shape: RoundedRectangleBorder(
                      borderRadius: BorderRadius.circular(12),
                    ),
                  ),
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }
}

class _SessionTile extends StatelessWidget {
  final String title;
  final String subtitle;
  final VoidCallback onTap;

  const _SessionTile({
    required this.title,
    required this.subtitle,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(12),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 10),
        child: Row(
          children: [
            const Icon(Icons.chat_bubble_outline_rounded,
                size: 18, color: AppTheme.textDim),
            const SizedBox(width: 12),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    title,
                    overflow: TextOverflow.ellipsis,
                    style: const TextStyle(
                      fontSize: 14,
                      fontWeight: FontWeight.w500,
                    ),
                  ),
                  if (subtitle.isNotEmpty)
                    Text(
                      subtitle,
                      overflow: TextOverflow.ellipsis,
                      style: const TextStyle(
                        fontSize: 12,
                        color: AppTheme.textDim,
                      ),
                    ),
                ],
              ),
            ),
            const Icon(Icons.chevron_right_rounded,
                size: 20, color: AppTheme.textDim),
          ],
        ),
      ),
    );
  }
}

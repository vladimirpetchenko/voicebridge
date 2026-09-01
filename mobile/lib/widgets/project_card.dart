import 'package:flutter/material.dart';

import '../models.dart';
import '../theme.dart';

/// Заголовок секции с числом элементов.
class SectionHeader extends StatelessWidget {
  final String title;
  final int count;

  const SectionHeader({super.key, required this.title, required this.count});

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

/// Пустая карточка-заглушка.
class EmptyCard extends StatelessWidget {
  final IconData icon;
  final String text;

  const EmptyCard({super.key, required this.icon, required this.text});

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

/// Строка скрытого проекта с кнопкой «Вернуть».
class HiddenProjectRow extends StatelessWidget {
  final String name;
  final VoidCallback onRestore;

  const HiddenProjectRow({super.key, required this.name, required this.onRestore});

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

/// Карточка проекта: запуск/остановка, список сессий, «Новая сессия».
class ProjectCard extends StatefulWidget {
  final Project project;
  final List<OpenCodeSession> sessions;
  final String Function(int) relTime;
  final VoidCallback onStart;
  final VoidCallback onStop;
  final VoidCallback onCreateSession;
  final VoidCallback onHide;
  final void Function(OpenCodeSession) onOpenSession;

  const ProjectCard({
    super.key,
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
  State<ProjectCard> createState() => _ProjectCardState();
}

class _ProjectCardState extends State<ProjectCard> {
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
                  SessionTile(
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

/// Строка сессии (название + относительное время).
class SessionTile extends StatelessWidget {
  final String title;
  final String subtitle;
  final VoidCallback onTap;

  const SessionTile({
    super.key,
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

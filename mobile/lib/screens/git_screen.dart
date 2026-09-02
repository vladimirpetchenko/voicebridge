import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../app_state.dart';
import '../models.dart';
import '../theme.dart';

/// Экран Git выбранной сессии (отдельный экран навигации): вкладки
/// «Изменения» (рабочее дерево), «История» (коммиты) и «Ветки».
class GitScreen extends StatefulWidget {
  const GitScreen({super.key});

  @override
  State<GitScreen> createState() => _GitScreenState();
}

class _GitScreenState extends State<GitScreen> {
  int _tab = 0; // 0 — изменения, 1 — история, 2 — ветки
  List<GitCommit> _commits = const [];
  bool _loadingCommits = false;
  bool _loadedCommits = false;

  List<GitBranch> _branches = const [];
  bool _loadingBranches = false;
  bool _loadedBranches = false;

  Future<void> _loadCommits() async {
    setState(() => _loadingCommits = true);
    final commits = await context.read<AppController>().getGitCommits();
    if (!mounted) return;
    setState(() {
      _commits = commits;
      _loadingCommits = false;
      _loadedCommits = true;
    });
  }

  Future<void> _loadBranches() async {
    setState(() => _loadingBranches = true);
    final branches = await context.read<AppController>().getGitBranches();
    if (!mounted) return;
    setState(() {
      _branches = branches;
      _loadingBranches = false;
      _loadedBranches = true;
    });
  }

  String _basename(String p) {
    final parts = p.split(RegExp(r'[\\/]'));
    return parts.isEmpty ? p : parts.last;
  }

  String _dirname(String p) {
    final i = p.lastIndexOf(RegExp(r'[\\/]'));
    return i < 0 ? '' : p.substring(0, i);
  }

  List<_GitGroup> _groupByDir(List<GitFileChange> changes) {
    final map = <String, List<GitFileChange>>{};
    for (final c in changes) {
      map.putIfAbsent(_dirname(c.path), () => []).add(c);
    }
    final dirs = map.keys.toList()
      ..sort((a, b) {
        if (a.isEmpty) return -1;
        if (b.isEmpty) return 1;
        return a.compareTo(b);
      });
    return [
      for (final dir in dirs)
        () {
          final items = map[dir]!..sort((a, b) => a.path.compareTo(b.path));
          return _GitGroup(
            dir: dir,
            changes: items,
            additions: items.fold<int>(0, (s, c) => s + c.additions),
            deletions: items.fold<int>(0, (s, c) => s + c.deletions),
          );
        }(),
    ];
  }

  Widget _fileTile(BuildContext context, GitFileChange c) {
    return _FileTile(
      name: _basename(c.path),
      dir: '',
      status: c.status,
      additions: c.additions,
      deletions: c.deletions,
      onTap: () {
        Navigator.of(context).push(
          MaterialPageRoute(builder: (_) => GitDiffScreen(change: c)),
        );
      },
    );
  }

  @override
  Widget build(BuildContext context) {
    final controller = context.watch<AppController>();
    final changes = controller.gitChanges;
    final branch = controller.gitBranch;
    final adds = changes.fold<int>(0, (s, c) => s + c.additions);
    final dels = changes.fold<int>(0, (s, c) => s + c.deletions);
    final groups = _groupByDir(changes);

    return Scaffold(
      appBar: AppBar(
        title: Text(
          branch.isNotEmpty ? branch : 'Git',
          overflow: TextOverflow.ellipsis,
        ),
        actions: [
          IconButton(
            tooltip: 'Обновить',
            icon: const Icon(Icons.refresh_rounded),
            onPressed: () {
              context.read<AppController>().refreshGitChanges();
              if (_tab == 1) {
                _loadCommits();
              } else if (_tab == 2) {
                _loadBranches();
              }
            },
          ),
        ],
        bottom: PreferredSize(
          preferredSize: const Size.fromHeight(56),
          child: Padding(
            padding: const EdgeInsets.fromLTRB(16, 0, 16, 10),
            child: _TabSwitcher(
              tab: _tab,
              onChanged: (t) {
                setState(() => _tab = t);
                if (t == 1 && !_loadedCommits) _loadCommits();
                if (t == 2 && !_loadedBranches) _loadBranches();
              },
            ),
          ),
        ),
      ),
      body: _tab == 0
          ? _buildChanges(context, changes, adds, dels, groups)
          : _tab == 1
              ? _buildCommits(context)
              : _buildBranches(context),
    );
  }

  Widget _buildChanges(
    BuildContext context,
    List<GitFileChange> changes,
    int adds,
    int dels,
    List<_GitGroup> groups,
  ) {
    return RefreshIndicator(
      onRefresh: () => context.read<AppController>().refreshGitChanges(),
      child: changes.isEmpty
          ? ListView(
              physics: const AlwaysScrollableScrollPhysics(),
              children: const [
                SizedBox(height: 120),
                Icon(Icons.account_tree_outlined,
                    size: 40, color: AppTheme.textDim),
                SizedBox(height: 12),
                Center(
                  child: Text(
                    'Нет изменений',
                    style: TextStyle(color: AppTheme.textDim),
                  ),
                ),
              ],
            )
          : ListView(
              physics: const AlwaysScrollableScrollPhysics(),
              padding: const EdgeInsets.fromLTRB(8, 8, 8, 24),
              children: [
                Padding(
                  padding: const EdgeInsets.fromLTRB(12, 2, 12, 8),
                  child: Text(
                    '+$adds −$dels · ${changes.length} файлов',
                    style: const TextStyle(fontSize: 12, color: AppTheme.textDim),
                  ),
                ),
                for (final g in groups)
                  if (g.dir.isEmpty)
                    ...g.changes.map((c) => _fileTile(context, c))
                  else
                    _FolderGroup(
                      dir: g.dir,
                      additions: g.additions,
                      deletions: g.deletions,
                      children: g.changes.map((c) => _fileTile(context, c)).toList(),
                    ),
              ],
            ),
    );
  }

  Widget _buildCommits(BuildContext context) {
    if (_loadingCommits && _commits.isEmpty) {
      return const Center(child: CircularProgressIndicator(strokeWidth: 2));
    }
    if (_commits.isEmpty) {
      return RefreshIndicator(
        onRefresh: _loadCommits,
        child: ListView(
          physics: const AlwaysScrollableScrollPhysics(),
          children: const [
            SizedBox(height: 120),
            Icon(Icons.history_rounded, size: 40, color: AppTheme.textDim),
            SizedBox(height: 12),
            Center(
              child: Text(
                'Нет коммитов',
                style: TextStyle(color: AppTheme.textDim),
              ),
            ),
          ],
        ),
      );
    }
    return RefreshIndicator(
      onRefresh: _loadCommits,
      child: ListView.builder(
        physics: const AlwaysScrollableScrollPhysics(),
        padding: const EdgeInsets.fromLTRB(8, 8, 8, 24),
        itemCount: _commits.length,
        itemBuilder: (context, i) {
          final c = _commits[i];
          return _CommitTile(
            commit: c,
            onTap: () {
              Navigator.of(context).push(
                MaterialPageRoute(builder: (_) => GitCommitScreen(commit: c)),
              );
            },
          );
        },
      ),
    );
  }

  Widget _buildBranches(BuildContext context) {
    if (_loadingBranches && _branches.isEmpty) {
      return const Center(child: CircularProgressIndicator(strokeWidth: 2));
    }
    if (_branches.isEmpty) {
      return RefreshIndicator(
        onRefresh: _loadBranches,
        child: ListView(
          physics: const AlwaysScrollableScrollPhysics(),
          children: const [
            SizedBox(height: 120),
            Icon(Icons.account_tree_rounded, size: 40, color: AppTheme.textDim),
            SizedBox(height: 12),
            Center(
              child: Text(
                'Нет веток',
                style: TextStyle(color: AppTheme.textDim),
              ),
            ),
          ],
        ),
      );
    }
    return RefreshIndicator(
      onRefresh: _loadBranches,
      child: ListView.builder(
        physics: const AlwaysScrollableScrollPhysics(),
        padding: const EdgeInsets.fromLTRB(8, 8, 8, 24),
        itemCount: _branches.length,
        itemBuilder: (context, i) => _BranchTile(branch: _branches[i]),
      ),
    );
  }
}

/// Переключатель вкладок «Изменения»/«История».
class _TabSwitcher extends StatelessWidget {
  final int tab;
  final ValueChanged<int> onChanged;

  const _TabSwitcher({required this.tab, required this.onChanged});

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(3),
      decoration: BoxDecoration(
        color: AppTheme.surface2,
        borderRadius: BorderRadius.circular(12),
      ),
      child: Row(
        children: [
          _tabButton(context, 0, 'Изменения'),
          _tabButton(context, 1, 'История'),
          _tabButton(context, 2, 'Ветки'),
        ],
      ),
    );
  }

  Widget _tabButton(BuildContext context, int value, String label) {
    final active = tab == value;
    return Expanded(
      child: InkWell(
        borderRadius: BorderRadius.circular(9),
        onTap: () => onChanged(value),
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 150),
          padding: const EdgeInsets.symmetric(vertical: 8),
          decoration: BoxDecoration(
            color: active ? AppTheme.surface : Colors.transparent,
            borderRadius: BorderRadius.circular(9),
          ),
          alignment: Alignment.center,
          child: Text(
            label,
            style: TextStyle(
              fontSize: 13,
              fontWeight: FontWeight.w600,
              color: active ? AppTheme.accent : AppTheme.textDim,
            ),
          ),
        ),
      ),
    );
  }
}

/// Элемент списка веток: имя, текущая помечается бейджем.
class _BranchTile extends StatelessWidget {
  final GitBranch branch;

  const _BranchTile({required this.branch});

  @override
  Widget build(BuildContext context) {
    final current = branch.current;
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 10),
      child: Row(
        children: [
          Icon(
            current ? Icons.account_tree_rounded : Icons.account_tree_outlined,
            size: 18,
            color: current ? AppTheme.accent : AppTheme.textDim,
          ),
          const SizedBox(width: 12),
          Expanded(
            child: Text(
              branch.name,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(
                fontSize: 14,
                fontWeight: current ? FontWeight.w600 : FontWeight.w400,
                color: current ? AppTheme.accent : AppTheme.textPrimary,
              ),
            ),
          ),
          if (current)
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
              decoration: BoxDecoration(
                color: const Color(0x1F22D3EE),
                borderRadius: BorderRadius.circular(999),
              ),
              child: const Text(
                'текущая',
                style: TextStyle(
                  fontSize: 11,
                  color: AppTheme.accent,
                  fontWeight: FontWeight.w600,
                ),
              ),
            ),
        ],
      ),
    );
  }
}

/// Элемент списка коммитов: хэш, сообщение, автор и относительное время.
class _CommitTile extends StatelessWidget {
  final GitCommit commit;
  final VoidCallback onTap;

  const _CommitTile({required this.commit, required this.onTap});

  String get _shortHash =>
      commit.hash.length > 7 ? commit.hash.substring(0, 7) : commit.hash;

  @override
  Widget build(BuildContext context) {
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(12),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 10),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Text(
                  _shortHash,
                  style: const TextStyle(
                    color: AppTheme.accent,
                    fontSize: 12,
                    fontWeight: FontWeight.w600,
                  ),
                ),
                const SizedBox(width: 10),
                Expanded(
                  child: Text(
                    commit.message,
                    overflow: TextOverflow.ellipsis,
                    style: const TextStyle(fontSize: 13.5, fontWeight: FontWeight.w500),
                  ),
                ),
              ],
            ),
            const SizedBox(height: 3),
            Padding(
              padding: const EdgeInsets.only(left: 0),
              child: Row(
                children: [
                  Expanded(
                    child: Text(
                      commit.author,
                      overflow: TextOverflow.ellipsis,
                      style: const TextStyle(fontSize: 11, color: AppTheme.textDim),
                    ),
                  ),
                  const SizedBox(width: 8),
                  Text(
                    _relativeTime(commit.date),
                    style: const TextStyle(fontSize: 11, color: AppTheme.textDim),
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}

String _relativeTime(int unixSec) {
  if (unixSec <= 0) return '';
  final diff = DateTime.now().difference(
    DateTime.fromMillisecondsSinceEpoch(unixSec * 1000),
  );
  if (diff.inSeconds < 60) return 'только что';
  if (diff.inMinutes < 60) return '${diff.inMinutes} мин назад';
  if (diff.inHours < 24) return '${diff.inHours} ч назад';
  if (diff.inDays < 30) return '${diff.inDays} дн назад';
  final d = DateTime.fromMillisecondsSinceEpoch(unixSec * 1000);
  return '${d.day} ${_months[d.month - 1]} ${d.year}';
}

const _months = [
  'янв',
  'фев',
  'мар',
  'апр',
  'мая',
  'июн',
  'июл',
  'авг',
  'сен',
  'окт',
  'ноя',
  'дек',
];

/// Экран одного коммита: метаданные, список файлов и дифф.
class GitCommitScreen extends StatefulWidget {
  final GitCommit commit;

  const GitCommitScreen({super.key, required this.commit});

  @override
  State<GitCommitScreen> createState() => _GitCommitScreenState();
}

class _GitCommitScreenState extends State<GitCommitScreen> {
  GitCommitDetail? _detail;
  bool _loading = true;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    setState(() {
      _loading = true;
      _detail = null;
    });
    final detail = await context.read<AppController>().getGitCommit(widget.commit.hash);
    if (!mounted) return;
    setState(() {
      _detail = detail;
      _loading = false;
    });
  }

  String get _shortHash => widget.commit.hash.length > 7
      ? widget.commit.hash.substring(0, 7)
      : widget.commit.hash;

  @override
  Widget build(BuildContext context) {
    final detail = _detail;
    return Scaffold(
      appBar: AppBar(
        title: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(_shortHash, overflow: TextOverflow.ellipsis),
            Text(
              widget.commit.message,
              overflow: TextOverflow.ellipsis,
              style: const TextStyle(fontSize: 11, color: AppTheme.textDim),
            ),
          ],
        ),
      ),
      body: _loading
          ? const Center(child: CircularProgressIndicator(strokeWidth: 2))
          : detail == null
              ? const Center(
                  child: Text(
                    'Коммит недоступен',
                    style: TextStyle(color: AppTheme.textDim),
                  ),
                )
              : _CommitDetailView(detail: detail),
    );
  }
}

class _CommitDetailView extends StatelessWidget {
  final GitCommitDetail detail;

  const _CommitDetailView({required this.detail});

  Color _statusColor(String status) => switch (status) {
        'added' => AppTheme.accent2,
        'deleted' => AppTheme.danger,
        _ => const Color(0xFFFBBF24),
      };

  IconData _statusIcon(String status) => switch (status) {
        'added' => Icons.add_circle_outline_rounded,
        'deleted' => Icons.remove_circle_outline_rounded,
        _ => Icons.edit_note_rounded,
      };

  @override
  Widget build(BuildContext context) {
    final diff = GitDiff(
      path: '',
      status: 'modified',
      tooLarge: detail.tooLarge,
      diff: detail.diff,
    );
    final d = DateTime.fromMillisecondsSinceEpoch(detail.date * 1000);
    final dateStr = '${d.day} ${_months[d.month - 1]} ${d.year}, '
        '${d.hour.toString().padLeft(2, '0')}:${d.minute.toString().padLeft(2, '0')}';

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 8, 16, 12),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                detail.message,
                style: const TextStyle(fontSize: 15, fontWeight: FontWeight.w600),
              ),
              const SizedBox(height: 4),
              Text(
                '${detail.author} · $dateStr',
                style: const TextStyle(fontSize: 12, color: AppTheme.textDim),
              ),
            ],
          ),
        ),
        if (detail.files.isNotEmpty)
          Container(
            constraints: const BoxConstraints(maxHeight: 220),
            decoration: const BoxDecoration(
              border: Border(
                top: BorderSide(color: Color(0x14FFFFFF)),
                bottom: BorderSide(color: Color(0x14FFFFFF)),
              ),
            ),
            child: ListView.builder(
              shrinkWrap: true,
              itemCount: detail.files.length,
              itemBuilder: (context, i) {
                final f = detail.files[i];
                return Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 7),
                  child: Row(
                    children: [
                      Icon(_statusIcon(f.status), size: 16, color: _statusColor(f.status)),
                      const SizedBox(width: 10),
                      Expanded(
                        child: Text(
                          f.path,
                          overflow: TextOverflow.ellipsis,
                          style: const TextStyle(fontSize: 13),
                        ),
                      ),
                      const SizedBox(width: 8),
                      if (f.additions > 0)
                        Text(
                          '+${f.additions}',
                          style: const TextStyle(
                            color: AppTheme.accent2,
                            fontSize: 12,
                            fontWeight: FontWeight.w600,
                          ),
                        ),
                      if (f.deletions > 0) ...[
                        const SizedBox(width: 6),
                        Text(
                          '−${f.deletions}',
                          style: const TextStyle(
                            color: AppTheme.danger,
                            fontSize: 12,
                            fontWeight: FontWeight.w600,
                          ),
                        ),
                      ],
                    ],
                  ),
                );
              },
            ),
          ),
        Expanded(
          child: detail.diff.isEmpty
              ? const Center(
                  child: Text(
                    'Нет диффа (возможно, merge-коммит)',
                    style: TextStyle(color: AppTheme.textDim),
                  ),
                )
              : _DiffView(diff: diff),
        ),
      ],
    );
  }
}

class _GitGroup {
  final String dir;
  final List<GitFileChange> changes;
  final int additions;
  final int deletions;

  const _GitGroup({
    required this.dir,
    required this.changes,
    required this.additions,
    required this.deletions,
  });
}

/// Сворачиваемая группа файлов по папке.
class _FolderGroup extends StatelessWidget {
  final String dir;
  final int additions;
  final int deletions;
  final List<Widget> children;

  const _FolderGroup({
    required this.dir,
    required this.additions,
    required this.deletions,
    required this.children,
  });

  @override
  Widget build(BuildContext context) {
    return Theme(
      data: Theme.of(context).copyWith(dividerColor: Colors.transparent),
      child: ExpansionTile(
        initiallyExpanded: true,
        tilePadding: const EdgeInsets.symmetric(horizontal: 12),
        childrenPadding: const EdgeInsets.only(left: 8, bottom: 4),
        leading: const Icon(Icons.folder_rounded,
            size: 18, color: AppTheme.accent2),
        title: Row(
          children: [
            Expanded(
              child: Text(
                dir,
                overflow: TextOverflow.ellipsis,
                style: const TextStyle(
                  fontSize: 13,
                  fontWeight: FontWeight.w600,
                ),
              ),
            ),
            if (additions > 0)
              Text(
                '+$additions',
                style: const TextStyle(
                  color: AppTheme.accent2,
                  fontSize: 12,
                  fontWeight: FontWeight.w600,
                ),
              ),
            if (deletions > 0) ...[
              const SizedBox(width: 6),
              Text(
                '−$deletions',
                style: const TextStyle(
                  color: AppTheme.danger,
                  fontSize: 12,
                  fontWeight: FontWeight.w600,
                ),
              ),
            ],
          ],
        ),
        children: children,
      ),
    );
  }
}

/// Дифф одного файла (до/после) с подсветкой строк.
class GitDiffScreen extends StatefulWidget {
  final GitFileChange change;

  const GitDiffScreen({super.key, required this.change});

  @override
  State<GitDiffScreen> createState() => _GitDiffScreenState();
}

class _GitDiffScreenState extends State<GitDiffScreen> {
  GitDiff? _diff;
  bool _loading = true;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    setState(() {
      _loading = true;
      _diff = null;
    });
    final diff = await context.read<AppController>().getGitDiff(widget.change.path);
    if (!mounted) return;
    setState(() {
      _diff = diff;
      _loading = false;
    });
  }

  @override
  Widget build(BuildContext context) {
    final name = widget.change.path.split(RegExp(r'[\\/]')).last;
    return Scaffold(
      appBar: AppBar(
        title: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(name, overflow: TextOverflow.ellipsis),
            Text(
              widget.change.path,
              overflow: TextOverflow.ellipsis,
              style: const TextStyle(fontSize: 11, color: AppTheme.textDim),
            ),
          ],
        ),
      ),
      body: _loading
          ? const Center(child: CircularProgressIndicator(strokeWidth: 2))
          : _diff == null || _diff!.diff.isEmpty
              ? const Center(
                  child: Text(
                    'Дифф недоступен (возможно, бинарный файл)',
                    style: TextStyle(color: AppTheme.textDim),
                  ),
                )
              : _DiffView(diff: _diff!),
    );
  }
}

class _FileTile extends StatelessWidget {
  final String name;
  final String dir;
  final String status;
  final int additions;
  final int deletions;
  final VoidCallback onTap;

  const _FileTile({
    required this.name,
    required this.dir,
    required this.status,
    required this.additions,
    required this.deletions,
    required this.onTap,
  });

  Color get _statusColor => switch (status) {
        'added' || 'untracked' => AppTheme.accent2,
        'deleted' => AppTheme.danger,
        'renamed' => AppTheme.accent,
        _ => const Color(0xFFFBBF24),
      };

  IconData get _statusIcon => switch (status) {
        'added' || 'untracked' => Icons.add_circle_outline_rounded,
        'deleted' => Icons.remove_circle_outline_rounded,
        'renamed' => Icons.drive_file_rename_outline_rounded,
        _ => Icons.edit_note_rounded,
      };

  @override
  Widget build(BuildContext context) {
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(12),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 10),
        child: Row(
          children: [
            Icon(_statusIcon, size: 18, color: _statusColor),
            const SizedBox(width: 12),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    name,
                    overflow: TextOverflow.ellipsis,
                    style: const TextStyle(fontSize: 14, fontWeight: FontWeight.w500),
                  ),
                  if (dir.isNotEmpty)
                    Text(
                      dir,
                      overflow: TextOverflow.ellipsis,
                      style: const TextStyle(fontSize: 11, color: AppTheme.textDim),
                    ),
                ],
              ),
            ),
            const SizedBox(width: 8),
            if (additions > 0)
              Text(
                '+$additions',
                style: const TextStyle(
                  color: AppTheme.accent2,
                  fontSize: 12,
                  fontWeight: FontWeight.w600,
                ),
              ),
            if (deletions > 0) ...[
              const SizedBox(width: 6),
              Text(
                '−$deletions',
                style: const TextStyle(
                  color: AppTheme.danger,
                  fontSize: 12,
                  fontWeight: FontWeight.w600,
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }
}

class _DiffView extends StatelessWidget {
  final GitDiff diff;

  const _DiffView({required this.diff});

  @override
  Widget build(BuildContext context) {
    final lines = diff.diff.split('\n');
    final rows = _parse(lines);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (diff.tooLarge)
          Container(
            width: double.infinity,
            padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 6),
            color: const Color(0x26FBBF24),
            child: const Text(
              'Файл большой — показана часть.',
              style: TextStyle(color: Color(0xFFFBBF24), fontSize: 11),
            ),
          ),
        Expanded(
          child: ListView.builder(
            itemCount: rows.length,
            itemBuilder: (context, i) {
              final r = rows[i];
              return Container(
                color: _bgFor(r.cls),
                padding: const EdgeInsets.symmetric(vertical: 1),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    SizedBox(
                      width: 42,
                      child: Text(
                        r.old,
                        textAlign: TextAlign.right,
                        style: TextStyle(
                          color: r.cls == 'del'
                              ? AppTheme.danger
                              : AppTheme.textDim,
                          fontSize: 11,
                        ),
                      ),
                    ),
                    SizedBox(
                      width: 42,
                      child: Text(
                        r.newLine,
                        textAlign: TextAlign.right,
                        style: TextStyle(
                          color: r.cls == 'add'
                              ? AppTheme.accent2
                              : AppTheme.textDim,
                          fontSize: 11,
                        ),
                      ),
                    ),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text(
                        r.text.isEmpty ? ' ' : r.text,
                        style: TextStyle(
                          color: _colorFor(r.cls),
                          fontSize: 12,
                          height: 1.5,
                          fontFamily: 'FiraCode',
                        ),
                      ),
                    ),
                  ],
                ),
              );
            },
          ),
        ),
      ],
    );
  }

  Color _colorFor(String cls) {
    switch (cls) {
      case 'add':
        return AppTheme.accent2;
      case 'del':
        return AppTheme.danger;
      case 'hunk':
        return AppTheme.accent;
      case 'meta':
        return AppTheme.textDim;
      default:
        return AppTheme.textPrimary;
    }
  }

  Color? _bgFor(String cls) {
    switch (cls) {
      case 'add':
        return const Color(0x1426A67E);
      case 'del':
        return const Color(0x14FB6B6B);
      case 'hunk':
        return const Color(0x1222D3EE);
      default:
        return null;
    }
  }

  List<_DiffRow> _parse(List<String> lines) {
    final rows = <_DiffRow>[];
    var oldLine = 0;
    var newLine = 0;
    for (final line in lines) {
      if (line.startsWith('@@')) {
        final m = RegExp(r'@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@')
            .firstMatch(line);
        if (m != null) {
          oldLine = int.tryParse(m.group(1)!) ?? 0;
          newLine = int.tryParse(m.group(2)!) ?? 0;
        }
        rows.add(_DiffRow('', '', 'hunk', line));
      } else if (line.startsWith('diff') ||
          line.startsWith('index') ||
          line.startsWith('new file') ||
          line.startsWith('deleted file') ||
          line.startsWith('similarity') ||
          line.startsWith('rename') ||
          line.startsWith('---') ||
          line.startsWith('+++') ||
          line.startsWith(r'\ No newline')) {
        rows.add(_DiffRow('', '', 'meta', line));
      } else if (line.startsWith('+')) {
        rows.add(_DiffRow('', '$newLine', 'add', line));
        newLine++;
      } else if (line.startsWith('-')) {
        rows.add(_DiffRow('$oldLine', '', 'del', line));
        oldLine++;
      } else {
        rows.add(_DiffRow('$oldLine', '$newLine', 'ctx', line));
        oldLine++;
        newLine++;
      }
    }
    return rows;
  }
}

class _DiffRow {
  final String old;
  final String newLine;
  final String cls;
  final String text;

  const _DiffRow(this.old, this.newLine, this.cls, this.text);
}

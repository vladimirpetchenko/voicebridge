import 'dart:async';

import 'package:device_info_plus/device_info_plus.dart';
import 'package:flutter/foundation.dart';

import 'models.dart';
import 'settings_store.dart';
import 'ws_client.dart';

enum ConnStatus { disconnected, connecting, connected }

/// Глобальное состояние приложения: соединение, сессии, выбранная сессия и
/// содержимое чата. Подключён через `provider`.
class AppController extends ChangeNotifier {
  final SettingsStore settings = SettingsStore();
  final WsClient ws = WsClient();

  ConnStatus status = ConnStatus.disconnected;
  String? errorMessage;

  List<OpenCodeInstance> instances = [];
  List<Project> projects = [];
  List<String> hiddenProjects = [];

  OpenCodeInstance? selectedInstance;
  OpenCodeSession? selectedSession;

  final List<ConversationMessage> messages = [];
  final List<ToolAction> tools = [];
  final List<PermissionRequest> pendingPermissions = [];
  final List<QuestionRequest> pendingQuestions = [];
  bool busy = false;

  SessionUsage? usage;

  final List<GitFileChange> gitChanges = [];
  String gitBranch = '';

  StreamSubscription<WsEvent>? _sub;
  StreamSubscription<void>? _discSub;

  String? _uri;
  String? _token;
  bool _shouldReconnect = false;
  Timer? _reconnectTimer;
  int _reconnectAttempts = 0;

  String? get selectedSessionId => selectedSession?.id;

  /// Читает сохранённые адрес/токен и, если есть, подключается.
  Future<void> init() async {
    final uri = await settings.getUri();
    final token = await settings.getToken();
    if (uri != null && uri.isNotEmpty && token != null && token.isNotEmpty) {
      await connect(uri, token);
    }
  }

  Future<void> connect(String uri, String token) async {
    _uri = uri;
    _token = token;
    _shouldReconnect = true;
    _reconnectTimer?.cancel();
    status = ConnStatus.connecting;
    errorMessage = null;
    notifyListeners();
    try {
      await ws.connect(uri, token);
      await settings.save(uri, token);
      _subscribe();
      _reconnectAttempts = 0;
      status = ConnStatus.connected;
      notifyListeners();
      await refreshSessions();
      await refreshProjects();
      await refreshHidden();
      await registerDevice();
    } catch (e) {
      status = ConnStatus.disconnected;
      errorMessage = 'Не удалось подключиться: $e';
      notifyListeners();
    }
  }

  Future<void> disconnect() async {
    _shouldReconnect = false;
    _reconnectTimer?.cancel();
    await _sub?.cancel();
    _sub = null;
    await ws.disconnect();
    status = ConnStatus.disconnected;
    notifyListeners();
  }

  void _subscribe() {
    _sub?.cancel();
    _sub = ws.events.listen(_onEvent);
    _discSub?.cancel();
    _discSub = ws.onDisconnected.listen((_) => _onUnexpectedDisconnect());
  }

  void _onUnexpectedDisconnect() {
    if (!_shouldReconnect) return;
    status = ConnStatus.disconnected;
    errorMessage = 'Соединение потеряно — переподключение…';
    notifyListeners();
    final delay = Duration(seconds: (1 << _reconnectAttempts.clamp(0, 4)));
    _reconnectAttempts++;
    _reconnectTimer?.cancel();
    _reconnectTimer = Timer(delay, () {
      final uri = _uri;
      final token = _token;
      if (uri == null || token == null) return;
      connect(uri, token);
    });
  }

  void _onEvent(WsEvent event) {
    final sessionId = selectedSessionId;
    switch (event.name) {
      case 'opencode-user':
        if (event.data['sessionId'] != sessionId) return;
        busy = true;
        tools.clear();
        messages.add(ConversationMessage(
          role: 'user',
          text: event.data['text'] as String? ?? '',
        ));
        messages.add(const ConversationMessage(role: 'assistant', text: ''));
        notifyListeners();
        break;
      case 'opencode-delta':
        if (event.data['sessionId'] != sessionId) return;
        final text = event.data['text'] as String? ?? '';
        if (messages.isNotEmpty && messages.last.isAssistant) {
          final last = messages.last;
          messages[messages.length - 1] = ConversationMessage(
            role: last.role,
            text: last.text + text,
            reasoning: last.reasoning,
          );
        } else {
          messages.add(ConversationMessage(role: 'assistant', text: text));
        }
        notifyListeners();
        break;
      case 'opencode-reasoning-delta':
        if (event.data['sessionId'] != sessionId) return;
        final delta = event.data['text'] as String? ?? '';
        if (messages.isNotEmpty && messages.last.isAssistant) {
          final last = messages.last;
          messages[messages.length - 1] = ConversationMessage(
            role: last.role,
            text: last.text,
            reasoning: last.reasoning + delta,
          );
        } else {
          messages.add(
            ConversationMessage(role: 'assistant', text: '', reasoning: delta),
          );
        }
        notifyListeners();
        break;
      case 'opencode-done':
        if (event.data['sessionId'] != sessionId) return;
        busy = false;
        notifyListeners();
        refreshUsage();
        break;
      case 'opencode-error':
        if (event.data['sessionId'] != sessionId) return;
        busy = false;
        errorMessage = event.data['error'] as String?;
        notifyListeners();
        break;
      case 'opencode-tool':
        if (event.data['sessionId'] != sessionId) return;
        final action = ToolAction.fromJson(event.data);
        final idx = tools.indexWhere((t) => t.callId == action.callId);
        if (idx >= 0) {
          final prev = tools[idx];
          tools[idx] = ToolAction(
            callId: action.callId,
            name: action.name,
            state: action.state,
            input: action.input.isNotEmpty ? action.input : prev.input,
            output: action.output.isNotEmpty ? action.output : prev.output,
          );
        } else {
          tools.add(action);
        }
        notifyListeners();
        break;
      case 'opencode-permission':
        if (event.data['sessionId'] != sessionId) return;
        pendingPermissions.add(PermissionRequest.fromJson(event.data));
        notifyListeners();
        break;
      case 'opencode-question':
        if (event.data['sessionId'] != sessionId) return;
        pendingQuestions.add(QuestionRequest.fromJson(event.data));
        notifyListeners();
        break;
      case 'state-changed':
        // Десктоп изменил состояние (например, выбрана другая сессия) —
        // обновляем список сессий.
        refreshSessions();
        break;
      case 'git-changes':
        if (event.data['sessionId'] != sessionId) return;
        gitBranch = event.data['branch'] as String? ?? '';
        gitChanges
          ..clear()
          ..addAll(((event.data['changes'] as List<dynamic>?) ?? [])
              .map((e) => GitFileChange.fromJson(e as Map<String, dynamic>)));
        notifyListeners();
        break;
    }
  }

  Future<void> refreshSessions() async {
    try {
      final data = await ws.command('list_sessions');
      instances = (data as List<dynamic>? ?? [])
          .map((e) => OpenCodeInstance.fromJson(e as Map<String, dynamic>))
          .toList();
      notifyListeners();
    } catch (_) {
      // сессии пока не важны — соединение могло быть в процессе установки
    }
  }

  Future<void> refreshProjects() async {
    try {
      final data = await ws.command('list_projects');
      projects = (data as List<dynamic>? ?? [])
          .map((e) => Project.fromJson(e as Map<String, dynamic>))
          .toList();
      notifyListeners();
    } catch (_) {}
  }

  Future<void> startProject(String worktree) async {
    try {
      final data = await ws.command('start_project', {'worktree': worktree});
      projects = (data as List<dynamic>? ?? [])
          .map((e) => Project.fromJson(e as Map<String, dynamic>))
          .toList();
      notifyListeners();
      await refreshSessions();
    } catch (_) {}
  }

  Future<void> stopProject(String worktree) async {
    try {
      final data = await ws.command('stop_project', {'worktree': worktree});
      projects = (data as List<dynamic>? ?? [])
          .map((e) => Project.fromJson(e as Map<String, dynamic>))
          .toList();
      notifyListeners();
      await refreshSessions();
    } catch (_) {}
  }

  Future<void> refreshHidden() async {
    try {
      final data = await ws.command('get_state');
      if (data is Map<String, dynamic>) {
        hiddenProjects = ((data['hiddenProjects'] as List<dynamic>?) ?? [])
            .map((e) => e.toString())
            .toList();
        notifyListeners();
      }
    } catch (_) {}
  }

  Future<void> hideProject(String worktree) async {
    try {
      final data = await ws.command('hide_project', {'worktree': worktree});
      hiddenProjects = (data as List<dynamic>? ?? []).map((e) => e.toString()).toList();
      notifyListeners();
      await refreshSessions();
    } catch (_) {}
  }

  Future<void> unhideProject(String worktree) async {
    try {
      final data = await ws.command('unhide_project', {'worktree': worktree});
      hiddenProjects = (data as List<dynamic>? ?? []).map((e) => e.toString()).toList();
      notifyListeners();
      await refreshProjects();
      await refreshSessions();
    } catch (_) {}
  }

  Future<void> createSession(int port, String worktree) async {
    try {
      final data = await ws.command('create_session', {
        'port': port,
        'worktree': worktree,
        'title': '',
      });
      if (data is Map<String, dynamic>) {
        final sessionId = data['sessionId'] as String? ?? '';
        final title = data['title'] as String? ?? '';
        selectedInstance = OpenCodeInstance(
          id: worktree,
          name: worktree.split('/').last,
          port: port,
          sessions: const [],
        );
        selectedSession = OpenCodeSession(
          id: sessionId,
          title: title,
          directory: worktree,
          updatedAt: 0,
          model: '',
        );
        messages.clear();
        tools.clear();
        pendingPermissions.clear();
        pendingQuestions.clear();
        gitChanges.clear();
        busy = false;
        usage = null;
        notifyListeners();
        await loadConversation();
        await refreshUsage();
        await refreshSessions();
        await refreshProjects();
      }
    } catch (_) {}
  }

  /// Сообщает десктопу, какое устройство подключилось (сохранение пары).
  Future<void> registerDevice() async {
    try {
      final deviceId = await settings.getDeviceId();
      final deviceName = await _deviceName();
      await ws.command('register_device', {
        'deviceId': deviceId,
        'deviceName': deviceName,
      });
    } catch (_) {}
  }

  Future<String> _deviceName() async {
    try {
      final info = DeviceInfoPlugin();
      final d = await info.deviceInfo;
      final data = d.data;
      if (data.containsKey('model') && (data['model'] as String?)?.isNotEmpty == true) {
        return data['model'] as String;
      }
      if (data.containsKey('name') && (data['name'] as String?)?.isNotEmpty == true) {
        return data['name'] as String;
      }
    } catch (_) {}
    return 'Мобильное устройство';
  }

  Future<void> selectSession(OpenCodeInstance instance, OpenCodeSession session) async {
    selectedInstance = instance;
    selectedSession = session;
    messages.clear();
    tools.clear();
    pendingPermissions.clear();
    pendingQuestions.clear();
    gitChanges.clear();
    busy = false;
    usage = null;
    notifyListeners();

    await ws.command('select_session', {
      'instanceId': instance.id,
      'port': instance.port,
      'sessionId': session.id,
      'title': session.title,
      'model': session.model,
    });

    await loadConversation();
    await refreshUsage();
    await refreshGitChanges();
  }

  Future<void> loadConversation() async {
    final id = selectedSessionId;
    if (id == null) return;
    try {
      final args = <String, dynamic>{'sessionId': id};
      final port = selectedInstance?.port;
      if (port != null && port > 0) args['port'] = port;
      final data = await ws.command('get_conversation', args);
      messages
        ..clear()
        ..addAll((data as List<dynamic>? ?? [])
            .map((e) => ConversationMessage.fromJson(e as Map<String, dynamic>)));
      notifyListeners();
    } catch (_) {}
  }

  Future<void> refreshUsage() async {
    final id = selectedSessionId;
    if (id == null) return;
    try {
      final data = await ws.command('get_session_usage', {'sessionId': id});
      usage = data == null ? null : SessionUsage.fromJson(data as Map<String, dynamic>);
      notifyListeners();
    } catch (_) {}
  }

  Future<void> refreshGitChanges() async {
    final id = selectedSessionId;
    if (id == null) return;
    try {
      final data = await ws.command('get_git_changes', {'sessionId': id});
      if (data is Map<String, dynamic>) {
        final info = GitInfo.fromJson(data);
        gitBranch = info.branch;
        gitChanges
          ..clear()
          ..addAll(info.changes);
        notifyListeners();
      }
    } catch (_) {}
  }

  Future<GitDiff?> getGitDiff(String path) async {
    final id = selectedSessionId;
    if (id == null) return null;
    try {
      final data = await ws.command('get_git_diff', {
        'sessionId': id,
        'path': path,
      });
      if (data == null) return null;
      return GitDiff.fromJson(data as Map<String, dynamic>);
    } catch (_) {
      return null;
    }
  }

  Future<List<GitCommit>> getGitCommits() async {
    final id = selectedSessionId;
    if (id == null) return const [];
    try {
      final data = await ws.command('get_git_commits', {'sessionId': id});
      return (data as List<dynamic>? ?? [])
          .map((e) => GitCommit.fromJson(e as Map<String, dynamic>))
          .toList();
    } catch (_) {
      return const [];
    }
  }

  Future<GitCommitDetail?> getGitCommit(String hash) async {
    final id = selectedSessionId;
    if (id == null) return null;
    try {
      final data = await ws.command('get_git_commit', {
        'sessionId': id,
        'hash': hash,
      });
      if (data == null) return null;
      return GitCommitDetail.fromJson(data as Map<String, dynamic>);
    } catch (_) {
      return null;
    }
  }

  Future<List<GitBranch>> getGitBranches() async {
    final id = selectedSessionId;
    if (id == null) return const [];
    try {
      final data = await ws.command('get_git_branches', {'sessionId': id});
      return (data as List<dynamic>? ?? [])
          .map((e) => GitBranch.fromJson(e as Map<String, dynamic>))
          .toList();
    } catch (_) {
      return const [];
    }
  }

  Future<void> sendPrompt(String text) async {
    if (text.trim().isEmpty) return;
    final id = selectedSessionId;
    await ws.command('send_prompt',
        id == null ? {'text': text} : {'text': text, 'sessionId': id});
  }

  Future<void> abort() async {
    final id = selectedSessionId;
    await ws.command('abort', id == null ? null : {'sessionId': id});
  }

  Future<void> replyPermission(PermissionRequest req, String reply) async {
    pendingPermissions.removeWhere((p) => p.requestId == req.requestId);
    notifyListeners();
    await ws.command('reply_permission', {
      'port': req.port,
      'requestId': req.requestId,
      'reply': reply,
    });
  }

  Future<void> answerQuestion(QuestionRequest req, List<List<String>> answers) async {
    pendingQuestions.removeWhere((q) => q.requestId == req.requestId);
    notifyListeners();
    await ws.command('reply_question', {
      'port': req.port,
      'requestId': req.requestId,
      'answers': answers,
    });
  }

  Future<void> rejectQuestion(QuestionRequest req) async {
    pendingQuestions.removeWhere((q) => q.requestId == req.requestId);
    notifyListeners();
    await ws.command('reject_question', {
      'port': req.port,
      'requestId': req.requestId,
    });
  }

  @override
  void dispose() {
    _shouldReconnect = false;
    _reconnectTimer?.cancel();
    _sub?.cancel();
    _discSub?.cancel();
    ws.dispose();
    super.dispose();
  }
}

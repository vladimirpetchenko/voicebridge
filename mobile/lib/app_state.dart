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

  OpenCodeInstance? selectedInstance;
  OpenCodeSession? selectedSession;

  final List<ConversationMessage> messages = [];
  final List<ToolAction> tools = [];
  final List<PermissionRequest> pendingPermissions = [];
  final List<QuestionRequest> pendingQuestions = [];
  bool busy = false;

  SessionUsage? usage;

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
          messages[messages.length - 1] =
              ConversationMessage(role: last.role, text: last.text + text);
        } else {
          messages.add(ConversationMessage(role: 'assistant', text: text));
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
          tools[idx] = action;
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
  }

  Future<void> loadConversation() async {
    final id = selectedSessionId;
    if (id == null) return;
    try {
      final data = await ws.command('get_conversation', {'sessionId': id});
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

import 'dart:async';
import 'dart:convert';

import 'package:web_socket_channel/web_socket_channel.dart';

/// Событие, пришедшее от десктопа (`type: "event"`).
class WsEvent {
  final String name;
  final Map<String, dynamic> data;

  const WsEvent(this.name, this.data);
}

/// Ответ на команду (`type: "response"`).
class WsResponse {
  final String id;
  final bool ok;
  final dynamic data;
  final String? error;

  const WsResponse(this.id, this.ok, this.data, this.error);
}

/// Тонкий клиент WebSocket: команды с возрастающим `id`, сопоставление ответов
/// по `id`, раздача событий подписчикам по `name`.
class WsClient {
  WebSocketChannel? _channel;
  int _nextId = 1;
  final Map<String, Completer<dynamic>> _pending = {};

  final _events = StreamController<WsEvent>.broadcast();
  final _disconnected = StreamController<void>.broadcast();

  /// Поток событий от десктопа.
  Stream<WsEvent> get events => _events.stream;

  /// Срабатывает при неожиданном обрыве соединения (ошибка/закрытие).
  Stream<void> get onDisconnected => _disconnected.stream;

  bool get connected => _channel != null;

  /// Подключается к десктопу. `uri` — `ws://<ip>:<port>`, `token` — токен пары.
  Future<void> connect(String uri, String token) async {
    await disconnect();
    final full = Uri.parse(uri).replace(
      queryParameters: {'token': token},
    );
    final channel = WebSocketChannel.connect(full);
    // Ожидаем установки соединения (первое сообщение или ошибка).
    await channel.ready;
    _channel = channel;
    channel.stream.listen(
      _onData,
      onError: (_) => _onUnexpectedClose('соединение разорвано'),
      onDone: () => _onUnexpectedClose('соединение закрыто'),
      cancelOnError: false,
    );
  }

  void _onData(dynamic raw) {
    if (raw is! String) return;
    final Map<String, dynamic> msg;
    try {
      msg = jsonDecode(raw) as Map<String, dynamic>;
    } catch (_) {
      return;
    }
    final type = msg['type'];
    if (type == 'response') {
      final id = msg['id'] as String? ?? '';
      final completer = _pending.remove(id);
      if (completer == null) return;
      if (msg['ok'] == true) {
        completer.complete(msg['data']);
      } else {
        completer.completeError(msg['error'] ?? 'ошибка');
      }
    } else if (type == 'event') {
      final name = msg['name'] as String? ?? '';
      final data = (msg['data'] as Map<String, dynamic>?) ?? const {};
      _events.add(WsEvent(name, data));
    }
  }

  void _failAll(String reason) {
    for (final c in _pending.values) {
      c.completeError(reason);
    }
    _pending.clear();
  }

  void _onUnexpectedClose(String reason) {
    _channel = null;
    _failAll(reason);
    if (!_disconnected.isClosed) {
      _disconnected.add(null);
    }
  }

  /// Отправляет команду и возвращает `data` из ответа.
  Future<dynamic> command(String name, [Map<String, dynamic>? args]) async {
    final ch = _channel;
    if (ch == null) throw StateError('нет соединения');
    final id = (_nextId++).toString();
    final msg = <String, dynamic>{
      'type': 'command',
      'id': id,
      'name': name,
      ...?args,
    };
    final completer = Completer<dynamic>();
    _pending[id] = completer;
    ch.sink.add(jsonEncode(msg));
    return completer.future;
  }

  Future<void> disconnect() async {
    final ch = _channel;
    _channel = null;
    if (ch != null) {
      await ch.sink.close();
    }
    _failAll('соединение закрыто');
  }

  void dispose() {
    disconnect();
    _events.close();
    _disconnected.close();
  }
}

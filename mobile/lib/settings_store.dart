import 'dart:math';

import 'package:flutter_secure_storage/flutter_secure_storage.dart';

/// Хранит адрес десктопа и токен пары «мобилка ↔ десктоп» в secure storage.
class SettingsStore {
  static const _keyUri = 'desktop_uri';
  static const _keyToken = 'desktop_token';
  static const _keyDeviceId = 'device_id';

  final FlutterSecureStorage _storage = const FlutterSecureStorage(
    aOptions: AndroidOptions(),
  );

  /// URI десктопа вида `ws://<ip>:<port>` (без токена).
  Future<String?> getUri() => _storage.read(key: _keyUri);

  Future<String?> getToken() => _storage.read(key: _keyToken);

  /// Стабильный идентификатор этого устройства (генерируется один раз).
  Future<String> getDeviceId() async {
    final existing = await _storage.read(key: _keyDeviceId);
    if (existing != null && existing.isNotEmpty) return existing;
    final rnd = Random.secure();
    final id = List.generate(32, (_) => rnd.nextInt(16).toRadixString(16)).join();
    await _storage.write(key: _keyDeviceId, value: id);
    return id;
  }

  Future<void> save(String uri, String token) async {
    await _storage.write(key: _keyUri, value: uri);
    await _storage.write(key: _keyToken, value: token);
  }

  Future<void> clear() async {
    await _storage.delete(key: _keyUri);
    await _storage.delete(key: _keyToken);
  }
}

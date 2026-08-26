import 'package:flutter/material.dart';
import 'package:mobile_scanner/mobile_scanner.dart';
import 'package:provider/provider.dart';

import '../app_state.dart';

/// Экран пейринга: сканирование QR-кода с десктопа или ручной ввод адреса
/// и токена.
class PairingScreen extends StatefulWidget {
  const PairingScreen({super.key});

  @override
  State<PairingScreen> createState() => _PairingScreenState();
}

class _PairingScreenState extends State<PairingScreen> {
  final _addressController = TextEditingController();
  final _tokenController = TextEditingController();
  bool _scanning = false;
  bool _manual = false;

  @override
  void dispose() {
    _addressController.dispose();
    _tokenController.dispose();
    super.dispose();
  }

  void _connectFromPair(String uri, String token) {
    context.read<AppController>().connect(uri, token);
  }

  void _onScan(BarcodeCapture capture) {
    for (final barcode in capture.barcodes) {
      final value = barcode.rawValue ?? barcode.displayValue;
      if (value == null) continue;
      final uri = Uri.tryParse(value);
      if (uri == null || uri.scheme != 'ws' && uri.scheme != 'wss') continue;
      final token = uri.queryParameters['token'] ?? '';
      final base = uri.replace(queryParameters: {}).toString();
      setState(() => _scanning = false);
      _connectFromPair(base, token);
      return;
    }
  }

  void _submitManual() {
    final address = _addressController.text.trim();
    final token = _tokenController.text.trim();
    if (address.isEmpty || token.isEmpty) return;
    _connectFromPair(address, token);
  }

  @override
  Widget build(BuildContext context) {
    final controller = context.watch<AppController>();
    final connecting = controller.status == ConnStatus.connecting;

    return Scaffold(
      appBar: AppBar(title: const Text('VoiceBridge — подключение')),
      body: ListView(
        padding: const EdgeInsets.all(24),
        children: [
          const SizedBox(height: 8),
          const Icon(Icons.link, size: 48),
          const SizedBox(height: 16),
          const Text(
            'Подключите телефон к десктопу VoiceBridge.\n'
            'На десктопе: Настройки → Мобильный доступ → включите '
            '«Принимать команды» и отсканируйте QR-код.',
            textAlign: TextAlign.center,
          ),
          const SizedBox(height: 24),
          if (connecting)
            const Column(
              children: [
                CircularProgressIndicator(),
                SizedBox(height: 12),
                Text('Подключение…'),
              ],
            ),
          if (controller.errorMessage != null && !connecting)
            Padding(
              padding: const EdgeInsets.only(bottom: 16),
              child: Text(
                controller.errorMessage!,
                textAlign: TextAlign.center,
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
            ),
          const SizedBox(height: 8),
          if (_scanning)
            SizedBox(
              height: 300,
              child: ClipRRect(
                borderRadius: BorderRadius.circular(16),
                child: Stack(
                  alignment: Alignment.center,
                  children: [
                    MobileScanner(onDetect: _onScan),
                    const Positioned(
                      bottom: 16,
                      child: Text('Наведите на QR-код'),
                    ),
                  ],
                ),
              ),
            ),
          if (!_scanning && !_manual)
            FilledButton.icon(
              onPressed: () => setState(() => _scanning = true),
              icon: const Icon(Icons.qr_code_scanner),
              label: const Text('Сканировать QR-код'),
            ),
          if (!_scanning)
            TextButton(
              onPressed: () => setState(() => _manual = !_manual),
              child: Text(_manual ? 'Скрыть ручной ввод' : 'Ввести адрес вручную'),
            ),
          if (_manual && !_scanning) ...[
            const SizedBox(height: 8),
            TextField(
              controller: _addressController,
              decoration: const InputDecoration(
                labelText: 'Адрес десктопа (ws://ip:port)',
                border: OutlineInputBorder(),
              ),
              keyboardType: TextInputType.url,
            ),
            const SizedBox(height: 12),
            TextField(
              controller: _tokenController,
              decoration: const InputDecoration(
                labelText: 'Токен',
                border: OutlineInputBorder(),
              ),
            ),
            const SizedBox(height: 12),
            FilledButton(
              onPressed: _submitManual,
              child: const Text('Подключиться'),
            ),
          ],
        ],
      ),
    );
  }
}

import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../app_state.dart';
import 'pairing_screen.dart';
import 'sessions_screen.dart';

/// Выбирает экран в зависимости от состояния соединения.
class HomeScreen extends StatelessWidget {
  const HomeScreen({super.key});

  @override
  Widget build(BuildContext context) {
    final status = context.watch<AppController>().status;
    if (status == ConnStatus.connected) {
      return const SessionsScreen();
    }
    return const PairingScreen();
  }
}

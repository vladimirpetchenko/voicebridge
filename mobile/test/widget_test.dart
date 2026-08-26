import 'package:flutter_test/flutter_test.dart';
import 'package:voicebridge_mobile/models.dart';

void main() {
  test('OpenCodeInstance парсится из JSON', () {
    final instance = OpenCodeInstance.fromJson({
      'id': 'port-4149',
      'name': 'my-project',
      'port': 4149,
      'sessions': [
        {
          'id': 'ses_1',
          'title': 'Test',
          'directory': '/tmp/proj',
          'updatedAt': 123,
          'model': 'gpt-4o',
        },
      ],
    });
    expect(instance.port, 4149);
    expect(instance.sessions.length, 1);
    expect(instance.sessions.first.id, 'ses_1');
    expect(instance.sessions.first.model, 'gpt-4o');
  });

  test('ConversationMessage определяет роль', () {
    const m = ConversationMessage(role: 'assistant', text: 'hi');
    expect(m.isAssistant, isTrue);
  });
}

/// Типы, повторяющие `src/types.ts` десктопного приложения.
///
/// Поля совпадают с сериализацией бэкенда (camelCase, как в мосте Tauri).
library;

class OpenCodeSession {
  final String id;
  final String title;
  final String directory;
  final int updatedAt;
  final String model;

  const OpenCodeSession({
    required this.id,
    required this.title,
    required this.directory,
    required this.updatedAt,
    required this.model,
  });

  factory OpenCodeSession.fromJson(Map<String, dynamic> json) {
    return OpenCodeSession(
      id: json['id'] as String? ?? '',
      title: json['title'] as String? ?? '',
      directory: json['directory'] as String? ?? '',
      updatedAt: (json['updatedAt'] as num?)?.toInt() ?? 0,
      model: json['model'] as String? ?? '',
    );
  }
}

class OpenCodeInstance {
  final String id;
  final String name;
  final int port;
  final List<OpenCodeSession> sessions;

  const OpenCodeInstance({
    required this.id,
    required this.name,
    required this.port,
    required this.sessions,
  });

  factory OpenCodeInstance.fromJson(Map<String, dynamic> json) {
    final sessions = (json['sessions'] as List<dynamic>? ?? [])
        .map((e) => OpenCodeSession.fromJson(e as Map<String, dynamic>))
        .toList();
    return OpenCodeInstance(
      id: json['id'] as String? ?? '',
      name: json['name'] as String? ?? '',
      port: (json['port'] as num?)?.toInt() ?? 0,
      sessions: sessions,
    );
  }
}

class Project {
  final String id;
  final String worktree;
  final String name;
  final int updated;
  final bool running;
  final int port;

  const Project({
    required this.id,
    required this.worktree,
    required this.name,
    required this.updated,
    required this.running,
    required this.port,
  });

  factory Project.fromJson(Map<String, dynamic> json) {
    return Project(
      id: json['id'] as String? ?? '',
      worktree: json['worktree'] as String? ?? '',
      name: json['name'] as String? ?? '',
      updated: (json['updated'] as num?)?.toInt() ?? 0,
      running: json['running'] as bool? ?? false,
      port: (json['port'] as num?)?.toInt() ?? 0,
    );
  }
}

class OpenCodeTarget {
  final String instanceId;
  final int port;
  final String sessionId;
  final String title;

  const OpenCodeTarget({
    required this.instanceId,
    required this.port,
    required this.sessionId,
    required this.title,
  });

  factory OpenCodeTarget.fromJson(Map<String, dynamic> json) {
    return OpenCodeTarget(
      instanceId: json['instanceId'] as String? ?? '',
      port: (json['port'] as num?)?.toInt() ?? 0,
      sessionId: json['sessionId'] as String? ?? '',
      title: json['title'] as String? ?? '',
    );
  }
}

class ConversationMessage {
  final String role;
  final String text;

  const ConversationMessage({required this.role, required this.text});

  factory ConversationMessage.fromJson(Map<String, dynamic> json) {
    return ConversationMessage(
      role: json['role'] as String? ?? '',
      text: json['text'] as String? ?? '',
    );
  }

  bool get isAssistant => role == 'assistant';
}

class PermissionRequest {
  final String sessionId;
  final String requestId;
  final int port;
  final String permission;
  final List<String> patterns;

  const PermissionRequest({
    required this.sessionId,
    required this.requestId,
    required this.port,
    required this.permission,
    required this.patterns,
  });

  factory PermissionRequest.fromJson(Map<String, dynamic> json) {
    return PermissionRequest(
      sessionId: json['sessionId'] as String? ?? '',
      requestId: json['requestId'] as String? ?? '',
      port: (json['port'] as num?)?.toInt() ?? 0,
      permission: json['permission'] as String? ?? '',
      patterns: (json['patterns'] as List<dynamic>? ?? [])
          .map((e) => e.toString())
          .toList(),
    );
  }
}

class QuestionOption {
  final String label;
  final String description;

  const QuestionOption({required this.label, required this.description});

  factory QuestionOption.fromJson(Map<String, dynamic> json) {
    return QuestionOption(
      label: json['label'] as String? ?? '',
      description: json['description'] as String? ?? '',
    );
  }
}

class QuestionInfo {
  final String question;
  final String header;
  final List<QuestionOption> options;
  final bool multiple;
  final bool custom;

  const QuestionInfo({
    required this.question,
    required this.header,
    required this.options,
    required this.multiple,
    required this.custom,
  });

  factory QuestionInfo.fromJson(Map<String, dynamic> json) {
    return QuestionInfo(
      question: json['question'] as String? ?? '',
      header: json['header'] as String? ?? '',
      options: (json['options'] as List<dynamic>? ?? [])
          .map((e) => QuestionOption.fromJson(e as Map<String, dynamic>))
          .toList(),
      multiple: json['multiple'] as bool? ?? false,
      custom: json['custom'] as bool? ?? false,
    );
  }
}

class QuestionRequest {
  final String sessionId;
  final String requestId;
  final int port;
  final List<QuestionInfo> questions;

  const QuestionRequest({
    required this.sessionId,
    required this.requestId,
    required this.port,
    required this.questions,
  });

  factory QuestionRequest.fromJson(Map<String, dynamic> json) {
    return QuestionRequest(
      sessionId: json['sessionId'] as String? ?? '',
      requestId: json['requestId'] as String? ?? '',
      port: (json['port'] as num?)?.toInt() ?? 0,
      questions: (json['questions'] as List<dynamic>? ?? [])
          .map((e) => QuestionInfo.fromJson(e as Map<String, dynamic>))
          .toList(),
    );
  }
}

class SessionUsage {
  final int tokensInput;
  final int tokensOutput;
  final int tokensReasoning;
  final int tokensTotal;
  final double cost;
  final int contextLimit;
  final String model;

  const SessionUsage({
    required this.tokensInput,
    required this.tokensOutput,
    required this.tokensReasoning,
    required this.tokensTotal,
    required this.cost,
    required this.contextLimit,
    required this.model,
  });

  factory SessionUsage.fromJson(Map<String, dynamic> json) {
    return SessionUsage(
      tokensInput: (json['tokensInput'] as num?)?.toInt() ?? 0,
      tokensOutput: (json['tokensOutput'] as num?)?.toInt() ?? 0,
      tokensReasoning: (json['tokensReasoning'] as num?)?.toInt() ?? 0,
      tokensTotal: (json['tokensTotal'] as num?)?.toInt() ?? 0,
      cost: (json['cost'] as num?)?.toDouble() ?? 0,
      contextLimit: (json['contextLimit'] as num?)?.toInt() ?? 0,
      model: json['model'] as String? ?? '',
    );
  }
}

class ToolAction {
  final String callId;
  final String name;
  final String state;

  const ToolAction({
    required this.callId,
    required this.name,
    required this.state,
  });

  factory ToolAction.fromJson(Map<String, dynamic> json) {
    return ToolAction(
      callId: json['callId'] as String? ?? '',
      name: json['name'] as String? ?? '',
      state: json['state'] as String? ?? '',
    );
  }

  ToolAction copyWith({String? state}) {
    return ToolAction(callId: callId, name: name, state: state ?? this.state);
  }
}

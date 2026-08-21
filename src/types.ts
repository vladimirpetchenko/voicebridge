export type AppMode = "opencode" | "gui";
export type AppStatus = "idle" | "recording" | "processing" | "error";

export interface OpenCodeSession {
  id: string;
  title: string;
  directory: string;
  updatedAt: number;
}

export interface OpenCodeInstance {
  id: string;
  name: string;
  port: number;
  sessions: OpenCodeSession[];
}

export interface Project {
  id: string;
  worktree: string;
  name: string;
  updated: number;
  running: boolean;
  port: number;
}

export interface OpenCodeTarget {
  instanceId: string;
  port: number;
  sessionId: string;
  title: string;
}

export interface OpenCodeInstanceRef {
  id: string;
  port: number;
  name: string;
}

export interface ToolAction {
  callId: string;
  name: string;
  state: "running" | "done" | "failed";
}

export interface ConversationMessage {
  role: string;
  text: string;
}

export interface PermissionRequest {
  sessionId: string;
  requestId: string;
  port: number;
  permission: string;
  patterns: string[];
}

export interface QuestionOption {
  label: string;
  description: string;
}

export interface QuestionInfo {
  question: string;
  header: string;
  options: QuestionOption[];
  multiple: boolean;
  custom: boolean;
}

export interface QuestionRequest {
  sessionId: string;
  requestId: string;
  port: number;
  questions: QuestionInfo[];
}

export interface SessionInfo {
  title: string;
  project: string;
}

export interface SessionUsage {
  tokensInput: number;
  tokensOutput: number;
  tokensReasoning: number;
  tokensTotal: number;
  cost: number;
  contextLimit: number;
  model: string;
}

export interface WindowInfo {
  id: string;
  title: string;
  appName: string;
}

export interface SttModelInfo {
  id: string;
  name: string;
  sizeMb: number;
  engine: string;
  supported: boolean;
  description: string;
  downloaded: boolean;
}

export interface DownloadProgress {
  modelId: string;
  downloadedBytes: number;
  totalBytes: number;
  percent: number;
}

export interface AppState {
  mode: AppMode;
  status: AppStatus;
  statusMessage: string;
  recording: boolean;
  sensitivity: number;
  silenceTimeout: number;
  pasteMethod: string;
  pasteDelayMs: number;
  sendMode: string;
  language: string;
  selectedModel: string | null;
  transcript: string;
  response: string;
  recordingSessionId: string | null;
  selectedMicrophone: string | null;
  selectedSession: OpenCodeTarget | null;
  activeInstance: OpenCodeInstanceRef | null;
  selectedWindow: WindowInfo | null;
}

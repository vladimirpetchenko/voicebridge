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
  language: string;
  selectedModel: string | null;
  transcript: string;
  response: string;
  selectedMicrophone: string | null;
  selectedSession: OpenCodeTarget | null;
  activeInstance: OpenCodeInstanceRef | null;
  selectedWindow: WindowInfo | null;
}

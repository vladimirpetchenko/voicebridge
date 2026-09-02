export type AppStatus = "idle" | "recording" | "processing" | "error";

export interface OpenCodeSession {
  id: string;
  title: string;
  directory: string;
  updatedAt: number;
  model: string;
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
  reasoning?: string;
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

export interface MobileInfo {
  enabled: boolean;
  port: number;
  ip: string;
  token: string;
  uri: string;
  qrSvg: string;
}

export interface KnownDevice {
  id: string;
  name: string;
  lastSeen: number;
}

export interface GitFileChange {
  path: string;
  status: "modified" | "added" | "deleted" | "untracked" | "renamed";
  additions: number;
  deletions: number;
}

export interface GitInfo {
  branch: string;
  changes: GitFileChange[];
}

export interface GitDiff {
  path: string;
  status: string;
  tooLarge: boolean;
  diff: string;
}

export interface GitCommit {
  hash: string;
  author: string;
  date: number;
  message: string;
}

export interface GitCommitFile {
  path: string;
  status: string;
  additions: number;
  deletions: number;
}

export interface GitCommitDetail {
  hash: string;
  author: string;
  date: number;
  message: string;
  files: GitCommitFile[];
  diff: string;
  tooLarge: boolean;
}

export interface GitBranchInfo {
  name: string;
  current: boolean;
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
  status: AppStatus;
  statusMessage: string;
  recording: boolean;
  sensitivity: number;
  silenceTimeout: number;
  sendMode: string;
  hotkey: string;
  mobileEnabled: boolean;
  mobilePort: number;
  mobileToken: string;
  language: string;
  selectedModel: string | null;
  transcript: string;
  response: string;
  recordingSessionId: string | null;
  selectedMicrophone: string | null;
  selectedSession: OpenCodeTarget | null;
  opencodeModel: string | null;
  activeInstance: OpenCodeInstanceRef | null;
  hiddenProjects: string[];
}

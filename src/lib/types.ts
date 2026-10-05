// Shapes returned by the Rust backend. Keep in sync with src-tauri/src/*.rs.

export interface WorkspaceInfo {
  id: string;
  name: string;
  path: string;
  createdAt: string;
  schemaVersion: number;
}

export interface PageSummary {
  id: string;
  parentId: string | null;
  title: string;
  revision: number;
  isFavorite: boolean;
  deletedAt: string | null;
  updatedAt: string;
}

export interface Page {
  id: string;
  parentId: string | null;
  title: string;
  body: JsonNode;
  revision: number;
  isFavorite: boolean;
  createdAt: string;
  updatedAt: string;
}

/** A Tiptap/ProseMirror JSON node. */
export interface JsonNode {
  type: string;
  attrs?: Record<string, unknown>;
  content?: JsonNode[];
  marks?: { type: string; attrs?: Record<string, unknown> }[];
  text?: string;
}

export type TaskStatus = 'todo' | 'doing' | 'done';
export type TaskPriority = 'low' | 'medium' | 'high';

export interface Task {
  id: string;
  projectId: string | null;
  title: string;
  description: string;
  status: TaskStatus;
  priority: TaskPriority;
  dueDate: string | null;
  sourcePageId: string | null;
  revision: number;
  createdAt: string;
  updatedAt: string;
}

export interface NewTask {
  title: string;
  description?: string;
  status?: TaskStatus;
  priority?: TaskPriority;
  dueDate?: string;
  projectId?: string;
  sourcePageId?: string;
}

export interface TaskPatch {
  title?: string;
  description?: string;
  status?: TaskStatus;
  priority?: TaskPriority;
  /** Empty string clears the due date. */
  dueDate?: string;
  /** Empty string removes the project. */
  projectId?: string;
}

export interface Project {
  id: string;
  name: string;
  createdAt: string;
  updatedAt: string;
}

export type SearchHitKind = 'page' | 'task';

export interface SearchHit {
  kind: SearchHitKind;
  id: string;
  title: string;
  /** Matched terms are wrapped in [square brackets] by the backend. */
  snippet: string;
}

export interface Settings {
  theme: 'system' | 'light' | 'dark';
}

export interface BackupInfo {
  path: string;
  createdAt: string;
  pages: number;
  tasks: number;
}

/** Error shape produced by the backend's `AppError` serializer. */
export interface AppErrorPayload {
  code: 'validation' | 'not_found' | 'conflict' | 'no_workspace' | 'database' | 'io' | 'data';
  message: string;
}

// ---- assistant -------------------------------------------------------------

export interface AiConfig {
  endpoint: string;
  model: string;
  allowRemote: boolean;
}

export type AiReadiness = 'ready' | 'model_missing' | 'unreachable' | 'not_configured';

export interface AiStatus {
  config: AiConfig;
  state: AiReadiness;
}

export interface Citation {
  n: number;
  kind: 'page' | 'task';
  id: string;
  title: string;
}

export interface StoredMessage {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  citations: Citation[];
  createdAt: string;
}

export interface ConversationSummary {
  id: string;
  title: string;
  updatedAt: string;
}

export type ProposalKind = 'create_page' | 'edit_page' | 'task_changes';
export type ProposalStatus = 'pending' | 'applied' | 'rejected' | 'stale' | 'undone';

export interface Proposal {
  id: string;
  runId: string | null;
  kind: ProposalKind;
  targetPageId: string | null;
  baseRevision: number | null;
  status: ProposalStatus;
  summary: string;
  diffText: string;
  appliedRevision: number | null;
  createdAt: string;
  decidedAt: string | null;
}

export interface RunStarted {
  runId: string;
  conversationId: string | null;
}

export interface AiDoneEvent {
  runId: string;
  status: 'completed' | 'failed' | 'cancelled';
  conversationId: string | null;
  content: string | null;
  citations: Citation[];
  invalidCitations: number;
  missingNumbers: string[];
  errorCategory: string | null;
  message: string | null;
  proposals: number;
}

export interface AiToolEvent {
  runId: string;
  step: number;
  tool: string;
  ok: boolean;
  summary: string;
}

export interface AiProposalEvent {
  runId: string;
  proposal: Proposal;
}

export interface AiDeltaEvent {
  runId: string;
  text: string;
}

export interface ToolTrace {
  step: number;
  tool: string;
  ok: boolean;
  summary: string;
  errorCategory: string | null;
}

export interface RunSummary {
  id: string;
  kind: 'chat' | 'page_action';
  status: 'running' | 'completed' | 'cancelled' | 'failed';
  model: string;
  steps: number;
  errorCategory: string | null;
  durationMs: number | null;
  promptTokens: number | null;
  outputTokens: number | null;
  startedAt: string;
}

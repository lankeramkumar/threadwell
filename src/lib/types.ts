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
  /** Set for pages that mirror a linked source file. Those pages are read only. */
  sourceId: string | null;
}

export interface SourceInfo {
  id: string;
  name: string;
  rootPath: string;
  addedAt: string;
  lastSyncedAt: string | null;
  lastSummary: string | null;
  fileCount: number;
}

export interface SyncReport {
  added: number;
  updated: number;
  removed: number;
  unchanged: number;
  skipped: number;
  failed: number;
  failures: string[];
  truncated: boolean;
  stopped: boolean;
}

export interface Page {
  id: string;
  parentId: string | null;
  title: string;
  body: JsonNode;
  revision: number;
  isFavorite: boolean;
  aiExcluded: boolean;
  createdAt: string;
  updatedAt: string;
  sourceId: string | null;
  sourcePath: string | null;
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
/** One entry in the workspace list. `available` is false when its folder or database is missing. */
export interface WorkspaceListItem {
  name: string;
  path: string;
  active: boolean;
  available: boolean;
}

export interface AppErrorPayload {
  code: 'validation' | 'not_found' | 'conflict' | 'no_workspace' | 'database' | 'io' | 'data';
  message: string;
}

// ---- assistant -------------------------------------------------------------

export interface AiConfig {
  endpoint: string;
  model: string;
  allowRemote: boolean;
  embedModel: string;
  architecture: 'single' | 'multi';
  retrievalMode: 'lexical' | 'hybrid';
  weightLexical: number;
  weightVector: number;
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
  /** For a linked file: the section the answer drew on, such as "Lines 41–80". */
  section: string | null;
}

/** What a drop on the window did with each path. */
export interface DropReport {
  linked: string[];
  imported: string[];
  skipped: string[];
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

// ---- knowledge index ---------------------------------------------------------

export interface IndexStatus {
  embedded: number;
  total: number;
  model: string;
  running: boolean;
}

export interface AiIndexEvent {
  status: 'progress' | 'idle' | 'error';
  embedded: number;
  category: string | null;
}

// ---- meetings ---------------------------------------------------------------

export interface Meeting {
  id: string;
  pageId: string;
  title: string;
  status: 'imported' | 'processing' | 'processed' | 'failed';
  error: string | null;
  createdAt: string;
  segmentCount: number;
  proposalId: string | null;
}

export interface MeetingSegment {
  ord: number;
  startMs: number | null;
  speaker: string;
  text: string;
}

export interface MeetingClaim {
  ord: number;
  kind: 'summary' | 'decision' | 'question' | 'action';
  text: string;
  segmentOrds: number[];
  proposalId: string | null;
}

export interface MeetingDetail {
  meeting: Meeting;
  segments: MeetingSegment[];
  claims: MeetingClaim[];
}

export interface MeetingDoneEvent {
  meetingId: string;
  runId: string;
  status: 'processed' | 'failed' | 'cancelled';
  message: string | null;
  proposalId: string | null;
}

// ---- recipes ----------------------------------------------------------------

export type ScheduleKind = 'manual' | 'daily' | 'weekly';

export interface Recipe {
  id: string;
  name: string;
  prompt: string;
  scheduleKind: ScheduleKind;
  scheduleTime: string | null;
  weekday: number | null;
  timezone: string;
  enabled: boolean;
  nextRunAt: string | null;
  createdAt: string;
}

export interface RecipeInput {
  name: string;
  prompt: string;
  scheduleKind: ScheduleKind;
  scheduleTime: string | null;
  weekday: number | null;
  timezone: string;
  enabled: boolean;
}

export interface RecipeRun {
  id: string;
  trigger: 'manual' | 'schedule' | 'catch_up';
  scheduledFor: string | null;
  status: 'running' | 'completed' | 'failed';
  errorCategory: string | null;
  message: string | null;
  proposalId: string | null;
  startedAt: string;
  durationMs: number | null;
}

export interface RecipeDoneEvent {
  recipeRunId: string;
  status: 'completed' | 'failed';
  proposalId: string | null;
  message: string | null;
}

// ---- folder import ----------------------------------------------------------

export interface ScanItem {
  relativePath: string;
  size: number;
  /** new, imported (unchanged since import), changed, unsupported or too_large */
  status: 'new' | 'imported' | 'changed' | 'unsupported' | 'too_large';
}

export interface ScanReport {
  items: ScanItem[];
  truncated: boolean;
}

export interface ImportReport {
  imported: number;
  skipped: number;
  failed: { relativePath: string; reason: string }[];
  pageIds: string[];
}

// ---- attachments, history, sync, audio ----------------------------------------

export interface Attachment {
  id: string;
  pageId: string;
  fileName: string;
  size: number;
  sha256: string;
  createdAt: string;
}

export interface ConversationHit {
  conversationId: string;
  title: string;
  snippet: string;
  updatedAt: string;
}

export interface FolderSync {
  path: string;
  imported: number;
  changed: number;
  failed: number;
  error: string | null;
}

// ---- telemetry ----

export interface TelemetrySettings {
  localTraces: boolean;
}

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

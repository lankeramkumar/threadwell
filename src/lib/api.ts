// Typed wrappers around Tauri commands. Components call these, never `invoke` directly.
import { invoke } from '@tauri-apps/api/core';
import type {
  IndexStatus,
  Meeting,
  MeetingDetail,
  Recipe,
  RecipeInput,
  RecipeRun,
  AiConfig,
  AiStatus,
  ConversationSummary,
  Proposal,
  RunStarted,
  RunSummary,
  StoredMessage,
  ToolTrace,
  BackupInfo,
  JsonNode,
  NewTask,
  Page,
  PageSummary,
  Project,
  SearchHit,
  Settings,
  Task,
  TaskPatch,
  WorkspaceInfo,
} from './types';

export const api = {
  appStatus: () => invoke<{ workspace: WorkspaceInfo | null }>('app_status'),
  createWorkspace: (path: string, name: string, withSample: boolean) =>
    invoke<WorkspaceInfo>('create_workspace', { path, name, withSample }),
  openWorkspace: (path: string) => invoke<WorkspaceInfo>('open_workspace', { path }),

  listPages: () => invoke<PageSummary[]>('list_pages'),
  listTrash: () => invoke<PageSummary[]>('list_trash'),
  getPage: (id: string) => invoke<Page>('get_page', { id }),
  createPage: (title: string, parentId: string | null) => invoke<Page>('create_page', { title, parentId }),
  savePage: (id: string, title: string, body: JsonNode, expectedRevision: number) =>
    invoke<Page>('save_page', { id, title, body, expectedRevision }),
  movePage: (id: string, parentId: string | null) => invoke<void>('move_page', { id, parentId }),
  setPageFavorite: (id: string, favorite: boolean) => invoke<void>('set_page_favorite', { id, favorite }),
  trashPage: (id: string) => invoke<number>('trash_page', { id }),
  restorePage: (id: string) => invoke<void>('restore_page', { id }),
  backlinks: (id: string) => invoke<PageSummary[]>('backlinks', { id }),

  listProjects: () => invoke<Project[]>('list_projects'),
  createProject: (name: string) => invoke<Project>('create_project', { name }),
  listTasks: (projectId: string | null) => invoke<Task[]>('list_tasks', { projectId }),
  createTask: (input: NewTask) => invoke<Task>('create_task', { input }),
  updateTask: (id: string, patch: TaskPatch, expectedRevision: number) =>
    invoke<Task>('update_task', { id, patch, expectedRevision }),
  deleteTask: (id: string) => invoke<void>('delete_task', { id }),

  search: (query: string) => invoke<SearchHit[]>('search_workspace', { query }),
  rebuildSearchIndex: () => invoke<number>('rebuild_search_index'),

  getSettings: () => invoke<Settings>('get_settings'),
  setSetting: (key: string, value: string) => invoke<void>('set_setting', { key, value }),

  exportPagesMarkdown: (destDir: string) => invoke<number>('export_all_markdown', { destDir }),
  exportTasksCsv: (destFile: string) => invoke<number>('export_tasks_csv', { destFile }),
  importMarkdown: (srcPath: string, parentId: string | null) => invoke<Page>('import_markdown', { srcPath, parentId }),
  createBackup: (destDir: string) => invoke<BackupInfo>('create_backup', { destDir }),
  restoreBackup: (backupDir: string, destDir: string) =>
    invoke<WorkspaceInfo>('restore_backup', { backupDir, destDir }),
  aiGetStatus: () => invoke<AiStatus>('ai_get_status'),
  aiSaveConfig: (endpoint: string, model: string, allowRemote: boolean) =>
    invoke<AiConfig>('ai_save_config', { endpoint, model, allowRemote }),
  aiChatSend: (request: { runId: string; conversationId: string | null; message: string; pageId: string | null }) =>
    invoke<RunStarted>('ai_chat_send', { request }),
  aiPageAction: (request: { runId: string; pageId: string; action: string; language?: string; selectedText: string }) =>
    invoke<RunStarted>('ai_page_action', { request }),
  aiCancel: (runId: string) => invoke<boolean>('ai_cancel', { runId }),
  aiListConversations: () => invoke<ConversationSummary[]>('ai_list_conversations'),
  aiGetConversation: (id: string) => invoke<StoredMessage[]>('ai_get_conversation', { id }),
  aiListRuns: (limit?: number) => invoke<RunSummary[]>('ai_list_runs', { limit: limit ?? null }),
  aiRunTrace: (runId: string) => invoke<ToolTrace[]>('ai_run_trace', { runId }),
  aiListProposals: (runId: string | null) => invoke<Proposal[]>('ai_list_proposals', { runId }),
  aiApplyProposal: (id: string) => invoke<Proposal>('ai_apply_proposal', { id }),
  aiRejectProposal: (id: string) => invoke<Proposal>('ai_reject_proposal', { id }),
  aiUndoProposal: (id: string) => invoke<Proposal>('ai_undo_proposal', { id }),

  aiSaveRetrieval: (embedModel: string, mode: string, weightLexical: number, weightVector: number) =>
    invoke<AiConfig>('ai_save_retrieval', { embedModel, mode, weightLexical, weightVector }),
  aiSetPageExcluded: (id: string, excluded: boolean) => invoke<void>('ai_set_page_excluded', { id, excluded }),
  aiIndexStatus: () => invoke<IndexStatus>('ai_index_status'),
  aiIndexStart: () => invoke<boolean>('ai_index_start'),

  meetingsList: () => invoke<Meeting[]>('meetings_list'),
  meetingsGet: (id: string) => invoke<MeetingDetail>('meetings_get', { id }),
  meetingsImportText: (title: string, text: string) => invoke<Meeting>('meetings_import_text', { title, text }),
  meetingsImportFile: (path: string) => invoke<Meeting>('meetings_import_file', { path }),
  meetingsImportAudio: (path: string) => invoke<void>('meetings_import_audio', { path }),
  meetingsProcess: (id: string) => invoke<{ runId: string }>('meetings_process', { id }),

  recipesList: () => invoke<Recipe[]>('recipes_list'),
  recipesCreate: (input: RecipeInput) => invoke<string>('recipes_create', { input }),
  recipesUpdate: (id: string, input: RecipeInput) => invoke<void>('recipes_update', { id, input }),
  recipesDelete: (id: string) => invoke<void>('recipes_delete', { id }),
  recipeRunsList: (recipeId: string) => invoke<RecipeRun[]>('recipe_runs_list', { recipeId }),
  recipeRunNow: (recipeId: string) => invoke<string>('recipe_run_now', { recipeId }),
};

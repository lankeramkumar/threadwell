# Video guides

Three short guides, recorded from the running Threadwell desktop window. Each one is a silent MP4 with a caption bar, so
the steps below match what you see.

| Video                                                                      | Length                    | What it covers                                                                              |
| -------------------------------------------------------------------------- | ------------------------- | ------------------------------------------------------------------------------------------- |
| [01-getting-around.mp4](01-getting-around.mp4)                             | about 32 seconds          | The main screen, the sidebar, opening a page, Tasks and Settings                            |
| [02-linked-folders-and-questions.mp4](02-linked-folders-and-questions.mp4) | about 1 minute 40 seconds | Linking a project folder, reading its files, and asking the assistant a question about them |
| [03-several-workspaces.mp4](03-several-workspaces.mp4)                     | about 34 seconds          | Workspaces in Settings and the sidebar, and opening a page from another workspace           |

## 01 Getting around

1. Threadwell opens to the open workspace. Notes, tasks and the assistant share one window.
2. The sidebar lists your pages, Tasks, Meetings, Sources, Recipes, Trash and Settings.
3. Open a page to read or edit it. Changes save automatically.
4. Tasks shows tasks as a table or a board. A task has no due date until you set one.
5. Settings holds the workspace location, appearance, import and export, backup and the assistant setup.
6. Home shows your recent pages.

## 02 Linked folders and questions

1. Click **Sources**, then **Link a folder…** and choose the folder.
2. Each linked folder shows its file count and when it was last synced.
3. **Show files** lists the readable files. Each one is a read-only page named by its path.
4. Open a linked file. The note at the top says where the file is, and editing is not offered.
5. Type a question in the assistant box, for example _How are duplicate charges prevented?_, and press **Send**.
6. The answer names its source. Click the source to open that file.

## 03 Several workspaces

1. Each workspace has its own pages, tasks, meetings and assistant history.
2. **Settings** lists every workspace you have opened. The open one is marked.
3. Switch changes the open workspace. Create or open another from the same list.
4. In the sidebar, **Workspaces** lists each workspace. Other workspaces show their pages read only.
5. Opening a page from another workspace switches to that workspace first.

## How these were recorded

- The videos come from the running app. Only the Threadwell window is captured and driven. No mouse or keyboard input
  goes to the rest of the desktop.
- Two steps were shortened for the recording. The Windows folder picker can't be driven from the recording, so the sample
  project's path was passed to the app directly, which is what the picker does. A new workspace was also created directly
  in an empty folder, instead of through the first-run screen.
- The assistant answers came from the local model `qwen2.5:3b` through Ollama on the recording machine. Answers can vary
  slightly between runs.
- The demo workspace used for the recordings contains sample content only. It is not published.

/**
 * The desktop-shell payloads (autostart, app update), each mirrored by its own
 * Rust module; re-exported from `types.ts`, the contract.
 */
/** Mirrors `StartupStatus` and its `Status` in src-tauri/src/startup.rs. */
export interface StartupStatus {
  status:
    | 'unsupported'
    | 'disabled'
    | 'enabled'
    | 'disabledByWindows'
    | 'unknownApproval'
    | 'conflict';
  registered: boolean;
}
/** Mirrors `UpdateInfo` in src-tauri/src/updates.rs. */
export interface UpdateInfo {
  version: string;
  notes: string;
  /** RFC 3339 UTC publication time, when known. */
  date: string | null;
}
/** Mirrors `DownloadProgress` in src-tauri/src/desktop_updates.rs. */
export interface DownloadProgress {
  downloaded: number;
  total: number | null;
  stage: 'downloading' | 'verifying' | 'installing';
}

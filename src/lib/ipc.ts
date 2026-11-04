import { Channel, invoke } from '@tauri-apps/api/core';
import type {
  AgentId,
  AppSettings,
  DownloadProgress,
  LaunchOutcome,
  LocalAccounts,
  LaunchRequest,
  RecentSession,
  Snapshot,
  StartupStatus,
  UpdateInfo,
} from './types';

/**
 * Every command the native shell registers in `generate_handler!`
 * (src-tauri/src/desktop.rs), with its camelCase arguments and its result.
 * `args: undefined` means the command takes none; a Rust `Result<(), String>`
 * resolves as `void` and every `Err` rejects with its German message.
 * A new command goes here, into `REGISTERED` and into `generate_handler!`;
 * the contract test keeps the three in step.
 */
export interface Commands {
  snapshot: { args: undefined; result: Snapshot };
  /**
   * Starts a new detection and resolves at once with `detecting: true`; the
   * finished snapshot follows as the `traygent:snapshot` event.
   */
  refresh_detection: { args: undefined; result: Snapshot };
  /** Validates and stores the whole settings object; invalid → German `Err`. */
  save_settings: { args: { settings: AppSettings }; result: Snapshot };
  /** Validates, opens the terminal and records the recent session. */
  launch_session: { args: { request: LaunchRequest }; result: LaunchOutcome };
  remove_recent: { args: { id: string }; result: Snapshot };
  /** The undo of `remove_recent`: puts the removed entry back. */
  restore_recent: { args: { recent: RecentSession }; result: Snapshot };
  /** Native folder dialog; `null` when cancelled. */
  pick_folder: { args: { initial: string | null }; result: string | null };
  /** Opens a terminal that runs the catalog install command. */
  install_agent: { args: { agent: AgentId }; result: void };
  /** Opens the folder in Explorer. */
  open_folder: { args: { path: string }; result: void };
  /** Verifies the token against `/user`, vaults it and lists the repositories. */
  github_connect_token: { args: { token: string }; result: Snapshot };
  /** The local gh sign-ins for github.com; reads no token. */
  github_local_accounts: { args: undefined; result: LocalAccounts };
  /** Explicit consent: imports that gh sign-in's token, as above. */
  github_import_local: { args: { login: string }; result: Snapshot };
  /** Forgets the account and removes its token from the vault. */
  github_disconnect: { args: undefined; result: Snapshot };
  /**
   * Reads the listing again; resolves with `github.loading: true`, the
   * finished listing follows as the `traygent:snapshot` event.
   */
  github_refresh_repos: { args: undefined; result: Snapshot };
  autostart_status: { args: undefined; result: StartupStatus };
  set_autostart: { args: { enabled: boolean }; result: StartupStatus };
  open_startup_settings: { args: undefined; result: void };
  /** `null` when the installed version is current. */
  check_update: { args: undefined; result: UpdateInfo | null };
  /** Downloads, verifies and starts the checked update; Traygent then exits and restarts. */
  install_update: {
    args: { onProgress: Channel<DownloadProgress> };
    result: void;
  };
  /** Registered for the shell; the panel itself never hides the window. */
  hide_panel: { args: undefined; result: void };
}

export type Command = keyof Commands;
type Args<K extends Command> = Commands[K]['args'];

/** Runtime list of the map's keys; the record type keeps it exhaustive and exact. */
const REGISTERED: Record<Command, true> = {
  snapshot: true,
  refresh_detection: true,
  save_settings: true,
  launch_session: true,
  remove_recent: true,
  restore_recent: true,
  pick_folder: true,
  install_agent: true,
  open_folder: true,
  github_connect_token: true,
  github_local_accounts: true,
  github_import_local: true,
  github_disconnect: true,
  github_refresh_repos: true,
  autostart_status: true,
  set_autostart: true,
  open_startup_settings: true,
  check_update: true,
  install_update: true,
  hide_panel: true,
};
export const COMMAND_NAMES: readonly string[] = Object.keys(REGISTERED);

/** The only way the panel reaches the native shell: one typed Tauri command. */
export function call<K extends Command>(
  command: K,
  ...[args]: Args<K> extends undefined ? [] : [args: Args<K>]
): Promise<Commands[K]['result']> {
  return invoke<Commands[K]['result']>(command, args);
}

/** A channel for a command's streamed messages, e.g. `install_update` progress. */
export function channel<T>(onMessage: (message: T) => void): Channel<T> {
  return new Channel<T>(onMessage);
}

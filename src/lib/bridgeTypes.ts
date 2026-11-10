// The bridge contract: what the native shell and the browser demo both serve.
import type {
  AgentId,
  AppSettings,
  LaunchOptions,
  LaunchOutcome,
  LaunchRequest,
  LocalAccounts,
  RecentSession,
  Snapshot,
} from './types';

/** The domain commands both the native shell and the demo serve. */
export interface Bridge {
  /** The current snapshot (cached detection natively). */
  snapshot(): Promise<Snapshot>;
  /**
   * Starts a detection; resolves at once with `detecting: true`. The finished
   * snapshot arrives through `observe`.
   */
  refreshDetection(): Promise<Snapshot>;
  /** Stores the whole settings object; rejects with a German message. */
  saveSettings(settings: AppSettings): Promise<Snapshot>;
  launchSession(request: LaunchRequest): Promise<LaunchOutcome>;
  removeRecent(id: string): Promise<Snapshot>;
  restoreRecent(recent: RecentSession): Promise<Snapshot>;
  /** Stores new options and ports on a recent; rejects with a German message. */
  updateRecent(
    id: string,
    options: LaunchOptions,
    ports: string[],
  ): Promise<Snapshot>;
  /** A folder dialog; `null` when cancelled. */
  pickFolder(initial: string | null): Promise<string | null>;
  installAgent(agent: AgentId): Promise<void>;
  openFolder(path: string): Promise<void>;
  /** Verifies and vaults a personal access token; rejects with a German message. */
  githubConnectToken(token: string): Promise<Snapshot>;
  /** The local gh sign-ins; reads no token. */
  githubLocalAccounts(): Promise<LocalAccounts>;
  /** Imports that gh sign-in's token after the user's explicit „Übernehmen“. */
  githubImportLocal(login: string): Promise<Snapshot>;
  githubDisconnect(): Promise<Snapshot>;
  /**
   * Reads the listing again; resolves at once with `github.loading: true`,
   * the finished listing arrives through `observe`.
   */
  githubRefreshRepos(): Promise<Snapshot>;
  /**
   * Starts an agent CLI update run; resolves at once with `running`
   * statuses, the results arrive through `observe`. Rejects with a German
   * message while a run is in progress.
   */
  updateAgents(): Promise<Snapshot>;
}

/** Pushed snapshots, e.g. a finished detection; resolves to the unsubscribe. */
export type ObserveSnapshots = (
  callback: (snapshot: Snapshot) => void,
) => Promise<() => void>;

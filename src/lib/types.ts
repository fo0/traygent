/**
 * The IPC data model, mirrored by src-tauri/src/models.rs (camelCase JSON).
 * This file is the contract: the Rust side mirrors it field by field, the
 * golden fixture `fixtures/snapshot.default.json` (written by a Rust test)
 * must satisfy `Snapshot`. Product rules: docs/SPEC.md.
 */

/** The coding agents Traygent starts. */
export type AgentId = 'claude' | 'opencode' | 'codex';
/** Where a session runs; `wsl` is reserved and never ready yet. */
export type EnvId = 'local' | 'docker' | 'wsl';
/** Unified permission mode; every agent maps it to its own flags. */
export type PermissionId = 'ask' | 'auto' | 'bypass';
/** Per start only, never a default. */
export type SessionMode = 'new' | 'continue' | 'resume';
/** The window a session opens in. */
export type TerminalId = 'windowsTerminal' | 'console';
/** Result of a probe. */
export type ProbeState =
  'ready' | 'missing' | 'stopped' | 'unavailable' | 'unknown';

/** One selectable value; `id` `default` sends no flag. */
export interface Choice {
  id: string;
  label: string;
  hint?: string;
}

export interface PermissionChoice {
  id: PermissionId;
  label: string;
  hint: string;
  /** Skips every prompt; the panel shows it in the warning colour. */
  danger: boolean;
}

export interface InstallInfo {
  /** Best-practice PowerShell one-liner, shown and copyable. */
  command: string;
  docsUrl: string;
  /** Update command, when the agent has one. */
  update?: string;
  /** Prerequisite shown next to the command, e.g. „Node.js 20+“. */
  requires?: string;
}

/** Static per-agent capabilities, sent by Rust so the panel renders generically. */
export interface AgentCatalogEntry {
  id: AgentId;
  label: string;
  /** First entry is `default` (no model flag). */
  models: Choice[];
  /** Any model string is allowed besides the listed ones. */
  freeModel: boolean;
  /** `[]` = no thinking level; `default` sends no flag. */
  efforts: Choice[];
  permissions: PermissionChoice[];
  /** Supported session modes; always contains `new`. */
  sessions: SessionMode[];
  /** Environments it can run in. */
  environments: EnvId[];
  install: InstallInfo;
}

/** What a start uses; agent defaults and recent sessions store this. */
export interface LaunchOptions {
  model: string;
  effort: string;
  permission: PermissionId;
  /** `''` = none; split on whitespace, validated, each part one literal. */
  extraArgs: string;
}

/** A GitHub repository, `github.com/<owner>/<name>` (docs/adr/0005). */
export interface RepoRef {
  /** 1–39 of `A-Z a-z 0-9 -`, no leading or trailing `-`. */
  owner: string;
  /** 1–100 of `A-Z a-z 0-9 . _ -`, not `.`/`..`, without a `.git` suffix. */
  name: string;
}

export interface LaunchRequest {
  agent: AgentId;
  env: EnvId;
  /**
   * Absolute Windows folder path. With `repo`: `''` derives
   * `<cloneRoot>\<owner>\<name>`; a folder (a recent's restart) is reused
   * while it exists, else derived as with `''`.
   */
  folder: string;
  /**
   * Start in this repository: an existing clone folder is reused, otherwise
   * the session script clones it first.
   */
  repo?: RepoRef;
  options: LaunchOptions;
  session: SessionMode;
  /**
   * Published ports for a Docker session (`docker run -p`), normalized specs
   * like `127.0.0.1:3000:3000`; always `[]` for any other environment.
   * Port spec rules: docs/SPEC.md → Ports.
   */
  ports: string[];
}

export interface RecentSession {
  /** Stable per (lowercased folder, agent, env). */
  id: string;
  agent: AgentId;
  env: EnvId;
  folder: string;
  /** The repository the folder was cloned from, when started from one. */
  repo?: RepoRef;
  options: LaunchOptions;
  /** The ports the last start published (Docker only, else `[]`). */
  ports: string[];
  /** RFC 3339. */
  lastStartedAt: string;
  starts: number;
}

export interface AgentStatus {
  agent: AgentId;
  env: EnvId;
  state: ProbeState;
  version?: string;
  /** Resolved CLI path (local only), for the tooltip. */
  path?: string;
  /** Short German note, e.g. „CLI wird beim ersten Start installiert“. */
  detail?: string;
}

export interface EnvStatus {
  env: EnvId;
  state: ProbeState;
  version?: string;
  detail?: string;
}

export interface DockerSettings {
  /** Base image per agent. */
  images: Record<AgentId, string>;
  /** Forward the host `gh auth token` name-only as GH_TOKEN. */
  ghToken: boolean;
  /** Ports every new Docker session starts with; the composer can change them per start. */
  ports: string[];
}

/** A well-known dev-server port the panel offers as a checkbox. */
export interface PortPreset {
  /** Container and host port, e.g. 3000. */
  port: number;
  /** Frameworks that use it, e.g. „Next.js · Node“. */
  label: string;
}

export interface GithubSettings {
  /**
   * Folder repositories are cloned into as `<cloneRoot>\<owner>\<name>`;
   * `''` = `GithubState.defaultCloneRoot`.
   */
  cloneRoot: string;
}

export interface AppSettings {
  defaultAgent: AgentId;
  /** Always one of `environments`, never `wsl`. */
  defaultEnv: EnvId;
  /**
   * The environments the composer offers, in `local` · `docker` · `wsl`
   * order; at least one of them starts sessions (Lokal or Docker). A display
   * preference: recents keep their environment and restart unchanged.
   */
  environments: EnvId[];
  terminal: TerminalId;
  /**
   * Windows Terminal only: every session opens as a new tab of one shared
   * window (`wt -w traygent new-tab …`) instead of a window of its own.
   */
  sharedWindow: boolean;
  /** Defaults per agent. */
  agents: Record<AgentId, LaunchOptions>;
  docker: DockerSettings;
  github: GithubSettings;
}

/** How the connected account's token was obtained. */
export type GithubSource = 'token' | 'gh';

/** The one connected GitHub account; its token stays in the OS vault. */
export interface GithubAccount {
  login: string;
  /** Stable GitHub user id. */
  id: number;
  source: GithubSource;
}

/** One repository of the account's `/user/repos` listing. */
export interface RepoSummary {
  owner: string;
  name: string;
  private: boolean;
  /** RFC 3339 time of the last push, when GitHub reports one. */
  pushedAt?: string;
  /** Short description, for the tooltip. */
  description?: string;
}

export interface GithubState {
  /** Absent while no account is connected. */
  account?: GithubAccount;
  /** The account's repositories, newest push first, at most 1000; `[]` without one. */
  repos: RepoSummary[];
  /** True while the listing loads; a `traygent:snapshot` event follows. */
  loading: boolean;
  /** German error of the last listing, e.g. the token expired. */
  error?: string;
  /** RFC 3339 time of the last successful listing. */
  listedAt?: string;
  /** `%USERPROFILE%\Traygent`, resolved; the clone root while `cloneRoot` is `''`. */
  defaultCloneRoot: string;
}

/** One sign-in the local GitHub CLI holds for github.com. */
export interface LocalAccount {
  login: string;
  /** False when gh cannot hand out its token (e.g. an environment token, old gh). */
  importable: boolean;
  /** Already the connected account. */
  connected: boolean;
}

/** Result of `github_local_accounts`. */
export interface LocalAccounts {
  accounts: LocalAccount[];
  /** One German line when gh is missing or holds no sign-in. */
  message?: string;
}

/** Mirrors `Snapshot` in src-tauri/src/models.rs; replaced wholesale on every change. */
export interface Snapshot {
  /** Simulated browser-preview data; the native shell always sends false. */
  demo: boolean;
  settings: AppSettings;
  catalog: AgentCatalogEntry[];
  /** Port checkboxes for Docker sessions, in display order. */
  portPresets: PortPreset[];
  /** One per (agent, env) for `local` and `docker`. */
  agents: AgentStatus[];
  /** `local` always ready, `docker` probed, `wsl` unavailable (soon). */
  environments: EnvStatus[];
  /** Newest first, at most 20. */
  recents: RecentSession[];
  /** The GitHub account and its repository listing. */
  github: GithubState;
  /** True while a detection runs; a `traygent:snapshot` event follows. */
  detecting: boolean;
  /**
   * Increases with every native state change and every finished detection;
   * the panel drops a snapshot older than the one it shows. The demo counts
   * too.
   */
  revision: number;
  /**
   * Persistent German warning, e.g. the state file is read-only (newer
   * version, unreadable) so settings and recent sessions are not saved.
   */
  notice?: string;
  /** RFC 3339 time of the last finished detection. */
  checkedAt?: string;
}

export interface LaunchOutcome {
  recent: RecentSession;
  snapshot: Snapshot;
}

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

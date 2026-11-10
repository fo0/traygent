/**
 * The GitHub part of the IPC data model (docs/adr/0005), mirrored by
 * src-tauri/src/models/github.rs; re-exported from `types.ts`, the contract.
 */

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

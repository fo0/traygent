import type { Bridge } from './bridgeTypes';
import { errorMessage } from './format';
import type { Notices } from './notices.svelte';
import { LISTING_MAX_AGE, listingStale } from './target';
import type { LocalAccounts, Snapshot } from './types';

/** Key of the GitHub notices: a newer result replaces the older one. */
export const GITHUB_NOTICE = 'github';

export interface GithubDeps {
  bridge: Bridge;
  notices: Notices;
  /** Shows a snapshot unless an older one; see `Panel.accept`. */
  accept: (snapshot: Snapshot) => boolean;
  /** The snapshot the panel shows. */
  current: () => Snapshot | null;
}

/**
 * The account actions of Settings → GitHub and the listing refresh. They
 * apply at once (explicit buttons, UX rule 5) and live in the panel, so a
 * closed dialog never drops their result. Connect, import and disconnect
 * reject with the German message, which the dialog shows inline.
 */
export class GithubActions {
  /** A refresh request is on its way; the listing itself says `loading`. */
  refreshing = $state(false);
  #deps: GithubDeps;
  /** When the field last asked for a stale listing; at most once per 15 min. */
  #autoAt = Number.NEGATIVE_INFINITY;

  constructor(deps: GithubDeps) {
    this.#deps = deps;
  }

  #connected(snapshot: Snapshot) {
    this.#deps.accept(snapshot);
    const login = snapshot.github.account?.login;
    if (login)
      this.#deps.notices.show({
        key: GITHUB_NOTICE,
        text: `Konto ${login} verbunden.`,
        tone: 'subtle',
      });
  }

  async connectToken(token: string) {
    this.#connected(await this.#deps.bridge.githubConnectToken(token));
  }

  localAccounts(): Promise<LocalAccounts> {
    return this.#deps.bridge.githubLocalAccounts();
  }

  async importLocal(login: string) {
    this.#connected(await this.#deps.bridge.githubImportLocal(login));
  }

  async disconnect() {
    this.#deps.notices.clear(GITHUB_NOTICE);
    this.#deps.accept(await this.#deps.bridge.githubDisconnect());
  }

  /** „Aktualisieren“: reads the listing again; rejects with the German message. */
  async refresh() {
    if (this.refreshing) return;
    this.refreshing = true;
    try {
      this.#deps.accept(await this.#deps.bridge.githubRefreshRepos());
    } finally {
      this.refreshing = false;
    }
  }

  /**
   * The folder field got focus: a listing older than 15 minutes is read again,
   * once per 15 minutes even when that fails. A failure shows as a warning.
   */
  refreshIfStale(now = Date.now()) {
    const snapshot = this.#deps.current();
    if (!snapshot || this.refreshing || !listingStale(snapshot.github, now))
      return;
    if (now - this.#autoAt < LISTING_MAX_AGE) return;
    this.#autoAt = now;
    this.refresh().catch((cause) =>
      this.#deps.notices.show({
        key: GITHUB_NOTICE,
        text: errorMessage(cause),
        tone: 'warning',
      }),
    );
  }
}

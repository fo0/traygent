/*
 * The composer's one field for a folder or a GitHub repository
 * (docs/SPEC.md → Repositories, docs/adr/0005). The panel classifies while
 * the user types; Rust classifies again and has the last word. Shared test
 * vectors: fixtures/targets.json.
 */
import type { GithubState, RepoRef, RepoSummary } from './types';
import { WINDOWS_ABSOLUTE } from './validation';

export type Target =
  | { kind: 'empty' }
  | { kind: 'folder'; folder: string }
  | { kind: 'repo'; repo: RepoRef }
  | { kind: 'search'; query: string };

/** At most this many suggestions under the field. */
export const SUGGESTION_LIMIT = 8;
/** A listing older than this is read again when the field gets focus. */
export const LISTING_MAX_AGE = 15 * 60_000;

/** 1–39 of `A-Za-z0-9-`, no leading or trailing `-`. */
const OWNER = /^[A-Za-z0-9](?:[A-Za-z0-9-]{0,37}[A-Za-z0-9])?$/;
const NAME = /^[A-Za-z0-9._-]{1,100}$/;
const URL =
  /^(?:https?:\/\/)?(?:www\.)?github\.com\/([^/?#\s]+)\/([^/?#\s]+)(?:[/?#]\S*)?$/i;
const SSH = /^git@github\.com:([^/\s]+)\/([^/\s]+)$/i;
const BARE = /^([^/\s]+)\/([^/\s]+)$/;

/** A valid repository, the name without its `.git` suffix; null otherwise. */
export function repoRef(owner: string, rawName: string): RepoRef | null {
  const name = rawName.replace(/\.git$/, '');
  if (!OWNER.test(owner) || !NAME.test(name)) return null;
  if (name === '.' || name === '..') return null;
  return { owner, name };
}

/** Classifies the trimmed text of the folder field. */
export function classifyTarget(input: string): Target {
  const text = input.trim();
  if (!text) return { kind: 'empty' };
  if (WINDOWS_ABSOLUTE.test(text)) return { kind: 'folder', folder: text };
  const match = URL.exec(text) ?? SSH.exec(text) ?? BARE.exec(text);
  const repo = match ? repoRef(match[1]!, match[2]!) : null;
  return repo ? { kind: 'repo', repo } : { kind: 'search', query: text };
}

/** Why the native side would refuse this repository; empty when valid. */
export function repoProblem(repo: RepoRef) {
  return repoRef(repo.owner, repo.name)?.name === repo.name
    ? ''
    : 'Ungültiges Repository.';
}

/** `<clone root>\<owner>\<name>`, the folder a repository start uses. */
export function cloneFolder(root: string, repo: RepoRef) {
  return `${root.trim().replace(/[\\/]+$/, '')}\\${repo.owner}\\${repo.name}`;
}

/** `owner/name`, the field text of a repository. */
export const repoLabel = (repo: RepoRef) => `${repo.owner}/${repo.name}`;

/** Why a start cannot use this field text yet; empty when it can. */
export function targetProblem(target: Target) {
  if (target.kind === 'empty') return 'Ordner wählen';
  if (target.kind === 'search') return 'Ordner oder Repo wählen';
  return '';
}

/**
 * The listing's repositories whose `owner/name` holds every word of the
 * query, case-insensitive, in listing order (newest push first).
 */
export function matchRepos(
  repos: readonly RepoSummary[],
  query: string,
  limit = SUGGESTION_LIMIT,
): RepoSummary[] {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  const found: RepoSummary[] = [];
  for (const repo of repos) {
    if (found.length >= limit) break;
    const text = repoLabel(repo).toLowerCase();
    if (words.every((word) => text.includes(word))) found.push(repo);
  }
  return found;
}

/** A connected account whose listing is older than 15 minutes and not loading. */
export function listingStale(github: GithubState, now = Date.now()) {
  if (!github.account || github.loading) return false;
  const listed = github.listedAt ? Date.parse(github.listedAt) : NaN;
  return Number.isNaN(listed) || now - listed > LISTING_MAX_AGE;
}

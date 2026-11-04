/*
 * The „Zuletzt“ list as pure functions: newest first, one entry per
 * (folder, agent, env), at most `RECENT_LIMIT`. The native core owns the real
 * list; the demo and the panel's undo use these rules to mirror it.
 */
import type {
  AgentId,
  EnvId,
  LaunchOptions,
  LaunchRequest,
  RecentSession,
} from './types';

export const RECENT_LIMIT = 20;

/** Stable per (lowercased folder, agent, env), like the native id. */
export function recentId(folder: string, agent: AgentId, env: EnvId) {
  return `${env}:${agent}:${folder.trim().toLowerCase()}`;
}

const newestFirst = (a: RecentSession, b: RecentSession) =>
  Date.parse(b.lastStartedAt) - Date.parse(a.lastStartedAt);

/**
 * The entry a start records: a new one, or the bumped existing one. A
 * repository start passes the folder derived from it.
 */
export function recordStart(
  list: readonly RecentSession[],
  request: LaunchRequest,
  now: Date = new Date(),
): RecentSession {
  const id = recentId(request.folder, request.agent, request.env);
  const previous = list.find((recent) => recent.id === id);
  return {
    id,
    agent: request.agent,
    env: request.env,
    folder: request.folder.trim(),
    ...(request.repo && { repo: { ...request.repo } }),
    options: { ...request.options },
    ports: [...request.ports],
    lastStartedAt: now.toISOString(),
    starts: (previous?.starts ?? 0) + 1,
  };
}

/** Puts `recent` first, replacing the entry with its id; keeps the limit. */
export function upsertRecent(
  list: readonly RecentSession[],
  recent: RecentSession,
): RecentSession[] {
  return [recent, ...list.filter((item) => item.id !== recent.id)].slice(
    0,
    RECENT_LIMIT,
  );
}

export function withoutRecent(
  list: readonly RecentSession[],
  id: string,
): RecentSession[] {
  return list.filter((item) => item.id !== id);
}

/**
 * The undo of a removal, as natively (src-tauri/src/recents.rs → restore):
 * back at its place by start time under its recomputed id; an entry started
 * again since the removal is newer and wins. The oldest falls off beyond the
 * limit. Null for an entry without a readable time or start count.
 */
export function restoreRecent(
  list: readonly RecentSession[],
  recent: RecentSession,
): RecentSession[] | null {
  if (Number.isNaN(Date.parse(recent.lastStartedAt)) || recent.starts < 1)
    return null;
  const entry = {
    ...recent,
    id: recentId(recent.folder, recent.agent, recent.env),
  };
  if (list.some((item) => item.id === entry.id)) return [...list];
  return [...list, entry].sort(newestFirst).slice(0, RECENT_LIMIT);
}

/**
 * „Bearbeiten …“, as natively (src-tauri/src/service.rs → update_recent): new
 * options (trimmed) and ports on the entry; its place, start time and count
 * stay. Null when the id is no longer listed.
 */
export function editRecent(
  list: readonly RecentSession[],
  id: string,
  options: LaunchOptions,
  ports: readonly string[],
): RecentSession[] | null {
  if (!list.some((item) => item.id === id)) return null;
  const trimmed = {
    ...options,
    model: options.model.trim(),
    effort: options.effort.trim(),
    extraArgs: options.extraArgs.trim(),
  };
  return list.map((item) =>
    item.id === id ? { ...item, options: trimmed, ports: [...ports] } : item,
  );
}

/** Distinct folders of the recent sessions, newest first. */
export function recentFolders(
  list: readonly RecentSession[],
  limit = 8,
): string[] {
  const seen = new Set<string>();
  const folders: string[] = [];
  for (const recent of list) {
    const key = recent.folder.toLowerCase();
    if (seen.has(key)) continue;
    seen.add(key);
    folders.push(recent.folder);
    if (folders.length === limit) break;
  }
  return folders;
}

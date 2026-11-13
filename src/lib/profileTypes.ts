/*
 * Agent profiles of the IPC contract (#33): several sign-ins per agent CLI,
 * each in a config folder of its own that Rust derives from the id and keeps
 * in sync with the others, sign-in aside. Re-exported by `types.ts`; Rust
 * mirrors it in src-tauri/src/models.rs.
 */
import type { AgentId } from './types';

/**
 * An extra profile of an agent. `id`: `^[a-z0-9][a-z0-9-]{0,23}$`, never
 * `default`; `label`: trimmed, 1–24 characters. Ids and labels (ignoring
 * case) are unique per agent, the label „Standard“ is reserved, at most 8
 * per agent.
 */
export interface AgentProfile {
  id: string;
  label: string;
}

/** One profile as found on disk; `default` is the agent's own folder. */
export interface ProfileStatus {
  agent: AgentId;
  /** `default` or one of `settings.profiles[agent]`. */
  id: string;
  /** „Standard“ for `default`. */
  label: string;
  /** The profile's config folder, e.g. `C:\Users\x\.claude-team`. */
  dir: string;
  signedIn: boolean;
  /** The signed-in account, e.g. its e-mail address. */
  account?: string;
  organization?: string;
  /** RFC 3339 time of the last sync into this profile. */
  syncedAt?: string;
  /** German reason the last sync into this profile failed. */
  syncError?: string;
}

/**
 * What the profiles of an agent share on top of the built-in lists (#35):
 * extra directory and file names in the agent's config folder, linked or
 * copied like the built-in ones. Each name `^[A-Za-z0-9._-]{1,64}$`, no
 * trailing dot, no Windows device name, nothing starting with `.traygent`,
 * no name the sync already handles; unique (ignoring case) across both
 * lists, at most 16 per list. `{ dirs: [], files: [] }` by default and for
 * an agent without `profiles`.
 */
export interface ProfileShare {
  dirs: string[];
  files: string[];
}

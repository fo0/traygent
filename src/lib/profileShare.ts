/*
 * The extra names an agent's profiles share (#35) as pure functions: the
 * lists the two fields hold, and the rules they follow, mirroring the native
 * checks in src-tauri/src/profiles/share.rs, which have the last word.
 */
import type { AgentId, AppSettings, ProfileShare } from './types';

export type ShareKind = keyof ProfileShare;
export const MAX_SHARED = 16;

const NAME = /^[A-Za-z0-9._-]{1,64}$/;
/** `CON`, `NUL`, `COM0`–`COM9` …, with or without an extension (as Rust's `is_device`). */
const DEVICE = /^(?:con|prn|aux|nul|com[0-9]|lpt[0-9])(?:\.|$)/i;
/** Traygent's own files in a profile folder start with this. */
const OWN_PREFIX = '.traygent';
/**
 * What the sync already shares, copies or keeps per profile, as the native
 * side spells and splits them (`share.rs → name_lists`): the folders it
 * links and the files it syncs, shown under „Bereits abgeglichen“.
 */
export const BUILT_IN_SHARE: Readonly<ProfileShare> = {
  dirs: [
    'skills',
    'agents',
    'commands',
    'rules',
    'output-styles',
    'workflows',
    'agent-memory',
    'themes',
    'plugins',
    'projects',
    'file-history',
    'plans',
    'tasks',
    'paste-cache',
    'hooks',
    'todos',
    'session-env',
  ],
  files: ['CLAUDE.md', 'keybindings.json', 'history.jsonl', 'settings.json'],
};
const HANDLED = new Set(
  [...BUILT_IN_SHARE.dirs, ...BUILT_IN_SHARE.files].map((name) =>
    name.toLowerCase(),
  ),
);
/** The state and sign-in files and anything named after them, e.g. a backup copy. */
export const PROTECTED_PREFIXES = ['.claude.json', '.credentials.json'];
/** Account caches and logs: they belong to one sign-in and are never shared. */
export const NEVER_SHARED = new Set([
  'policy-limits.json',
  'policy-limits.json.stamp.json',
  'remote-settings.json',
  'cache',
  'statsig',
  'telemetry',
  'backups',
  'debug',
  'shell-snapshots',
  'ide',
  'local',
  'sessions',
  'jobs',
  'daemon',
]);
/** The names the sync handles itself, as the native side lists them. */
export const HANDLED_NAMES: ReadonlySet<string> = HANDLED;
const NOUNS: Record<ShareKind, string> = { dirs: 'Ordner', files: 'Dateien' };

export const emptyShare = (): ProfileShare => ({ dirs: [], files: [] });

/** The share of `agent`, empty when the settings have none. */
export const agentShare = (
  settings: Pick<AppSettings, 'profileShare'>,
  agent: AgentId,
): ProfileShare => settings.profileShare?.[agent] ?? emptyShare();

/** The names typed into a field: split at commas and whitespace, repeats once. */
export function parseShareNames(text: string) {
  return [...new Set(text.split(/[\s,]+/).filter(Boolean))];
}

/** A list as the field shows it. */
export const shareText = (names: readonly string[]) => names.join(', ');

/** Whether `name` can be a file or folder name inside a profile folder. */
function invalidName(name: string) {
  const lower = name.toLowerCase();
  return (
    !NAME.test(name) ||
    name.endsWith('.') ||
    DEVICE.test(name) ||
    lower.startsWith(OWN_PREFIX) ||
    PROTECTED_PREFIXES.some((prefix) => lower.startsWith(prefix)) ||
    NEVER_SHARED.has(lower)
  );
}

/**
 * Why each list of `share` is refused, as natively (`label` names the
 * agent); a list without a problem has no key. Names are checked folders
 * first; a name repeated across both lists, ignoring case, is the second
 * one's problem.
 */
export function shareProblems(
  share: ProfileShare,
  label: string,
): Partial<Record<ShareKind, string>> {
  const problems: Partial<Record<ShareKind, string>> = {};
  const seen = new Set<string>();
  for (const kind of ['dirs', 'files'] as const) {
    const list = share[kind];
    if (list.length > MAX_SHARED)
      problems[kind] =
        `Höchstens ${MAX_SHARED} weitere ${NOUNS[kind]} für ${label} möglich.`;
    for (const name of list) {
      const lower = name.toLowerCase();
      const problem =
        (invalidName(name) && 'ist ungültig') ||
        (HANDLED.has(lower) && 'wird bereits abgeglichen') ||
        (seen.has(lower) && 'ist doppelt vergeben');
      seen.add(lower);
      if (problem) problems[kind] ??= `Name „${name}“ für ${label} ${problem}.`;
    }
  }
  return problems;
}

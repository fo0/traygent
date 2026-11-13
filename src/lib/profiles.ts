/*
 * Agent profiles (#33) as pure functions: the id a typed name gives, the
 * rules a new profile and a saved list follow (mirroring the native checks,
 * which have the last word), and what the composer, „Zuletzt“ and the
 * settings read from a snapshot.
 */
import type {
  AgentId,
  AgentProfile,
  AppSettings,
  EnvId,
  LaunchRequest,
  ProfileStatus,
  Snapshot,
} from './types';

/** The agent's own config folder; never an extra profile's id. */
export const DEFAULT_PROFILE = 'default';
/** The label of `default`, reserved for it. */
export const STANDARD_LABEL = 'Standard';
export const MAX_PROFILES = 8;
/** A start or an edit names a profile the agent does not list, as natively. */
export const PROFILE_MISSING = 'Profil nicht gefunden.';
const MAX_LABEL_CHARS = 24;
const ID = /^[a-z0-9][a-z0-9-]{0,23}$/;
// eslint-disable-next-line no-control-regex
const CONTROL = /[\u0000-\u001f\u007f-\u009f]/;

/** German letters spelled out before the slug: „Büro“ gives `buero`. */
const GERMAN: Record<string, string> = { ä: 'ae', ö: 'oe', ü: 'ue', ß: 'ss' };

/**
 * The id (and folder suffix) a typed name gives: lowercased, ä ö ü ß spelled
 * out (ae oe ue ss), other diacritics dropped, every run of other characters
 * than `a-z 0-9` one `-`, trimmed of `-`, at most 24.
 */
export function profileId(name: string) {
  return name
    .normalize('NFC')
    .toLowerCase()
    .replace(/[äöüß]/g, (letter) => GERMAN[letter] ?? letter)
    .normalize('NFD')
    .replace(/\p{M}+/gu, '')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 24)
    .replace(/-+$/, '');
}

/** The folder Rust derives from the id, for the tooltip only. */
export function profileFolder(agent: AgentId, id: string) {
  const own = `%USERPROFILE%\\.${agent}`;
  return id === DEFAULT_PROFILE ? own : `${own}-${id}`;
}

/** The rules every label follows; `''` when `label` (trimmed) passes. */
function labelProblem(label: string) {
  if (!label) return 'Name fehlt.';
  if ([...label].length > MAX_LABEL_CHARS) return 'Höchstens 24 Zeichen.';
  if (CONTROL.test(label)) return 'Keine Steuerzeichen.';
  if (label.toLowerCase() === STANDARD_LABEL.toLowerCase())
    return `„${label}“ ist reserviert.`;
  return '';
}

const taken = (list: readonly AgentProfile[], id: string, label: string) =>
  list.some(
    (p) => p.id === id || p.label.toLowerCase() === label.toLowerCase(),
  );

/** Why `name` cannot become one more of `list`; `''` when it can. */
export function newProfileProblem(name: string, list: readonly AgentProfile[]) {
  const label = name.trim();
  const problem = labelProblem(label);
  if (problem) return problem;
  const id = profileId(label);
  if (!id) return 'Mindestens ein Buchstabe oder eine Ziffer.';
  if (id === DEFAULT_PROFILE) return `„${label}“ ist reserviert.`;
  if (taken(list, id, label)) return 'Profil gibt es schon.';
  if (list.length >= MAX_PROFILES) return `Höchstens ${MAX_PROFILES} Profile.`;
  return '';
}

/**
 * Why a saved profile is refused next to `others` (the profiles its id and
 * label must differ from), unprefixed; `''` when it passes.
 */
function savedProblem(
  { id, label }: AgentProfile,
  others: readonly AgentProfile[],
) {
  return (
    (label !== label.trim() && 'Ungültiger Name.') ||
    labelProblem(label) ||
    ((!ID.test(id) || id === DEFAULT_PROFILE) && 'Ungültige Kennung.') ||
    (taken(others, id, label) && 'Profil gibt es schon.') ||
    ''
  );
}

/**
 * Why `list[index]` is refused after a rename in place (the id and folder
 * stay): the rules of `profilesProblem` against every other profile,
 * unprefixed.
 */
export function renameProblem(list: readonly AgentProfile[], index: number) {
  const item = list[index];
  if (!item) return '';
  return savedProblem(
    item,
    list.filter((_, other) => other !== index),
  );
}

/** Why a saved list and its default are refused, as natively; `''` when valid. */
export function profilesProblem(list: readonly AgentProfile[], chosen: string) {
  if (list.length > MAX_PROFILES) return `Höchstens ${MAX_PROFILES} Profile.`;
  for (const [index, item] of list.entries()) {
    const problem = savedProblem(item, list.slice(0, index));
    if (problem) return `${item.label}: ${problem}`;
  }
  if (chosen !== DEFAULT_PROFILE && !list.some((p) => p.id === chosen))
    return 'Standard-Profil gibt es nicht.';
  return '';
}

/** Whether a start of `agent` in `env` sends a profile: Lokal only. */
export function profilesApply(
  snapshot: Pick<Snapshot, 'catalog'>,
  agent: AgentId,
  env: EnvId,
) {
  const entry = snapshot.catalog.find((item) => item.id === agent);
  return !!entry?.profiles && env === 'local';
}

/** Whether `id` names a profile of `agent` in `settings`. */
export function knownProfile(
  settings: Pick<AppSettings, 'profiles'>,
  agent: AgentId,
  id: string | undefined,
) {
  return (
    id === DEFAULT_PROFILE ||
    (!!id && !!settings.profiles?.[agent]?.some((p) => p.id === id))
  );
}

/** The saved default profile of `agent`, `default` when it is gone. */
export function defaultProfile(
  settings: Pick<AppSettings, 'profiles' | 'agentProfile'>,
  agent: AgentId,
) {
  const chosen = settings.agentProfile?.[agent];
  return knownProfile(settings, agent, chosen) && chosen
    ? chosen
    : DEFAULT_PROFILE;
}

/**
 * The profiles of `agent` in display order, „Standard“ first: the snapshot's
 * statuses, else (before Rust reported any) the saved list, not signed in.
 */
export function agentProfiles(
  snapshot: Pick<Snapshot, 'profiles' | 'settings'>,
  agent: AgentId,
): ProfileStatus[] {
  const found = (snapshot.profiles ?? []).filter((p) => p.agent === agent);
  if (found.length) return found;
  const base = { agent, dir: '', signedIn: false };
  return [
    { ...base, id: DEFAULT_PROFILE, label: STANDARD_LABEL },
    ...(snapshot.settings.profiles?.[agent] ?? []).map((p) => ({
      ...base,
      ...p,
    })),
  ];
}

/** The label of a profile id; the id itself for one no longer listed. */
export function profileLabel(
  snapshot: Pick<Snapshot, 'profiles' | 'settings'>,
  agent: AgentId,
  id: string | undefined,
) {
  if (!id || id === DEFAULT_PROFILE) return STANDARD_LABEL;
  return agentProfiles(snapshot, agent).find((p) => p.id === id)?.label ?? id;
}

/** The first character of a label, upper case: the profile's mark. */
export function profileInitial(label: string) {
  return ([...label.trim()][0] ?? '?').toUpperCase();
}

/**
 * Why a start's profile is refused, as natively (launch_prep.rs →
 * launch_profile, by `editedProfile`): only an id the agent does not list, in
 * Lokal; „Standard“ and every other environment start without one.
 */
export function launchProfileProblem(
  snapshot: Pick<Snapshot, 'settings'>,
  request: Pick<LaunchRequest, 'agent' | 'env' | 'profile'>,
) {
  const { agent, env, profile } = request;
  return editedProfile(snapshot.settings, agent, env, profile) === null
    ? PROFILE_MISSING
    : '';
}

/**
 * The „Profil“ choices of a recent's „Bearbeiten …“: „Standard“ and the
 * agent's profiles; none (no field) outside Lokal, or without an extra one
 * unless the entry still stores a profile (then just „Standard“, so the
 * change a save makes is visible).
 */
export function recentProfileItems(
  snapshot: Pick<Snapshot, 'catalog' | 'profiles' | 'settings'>,
  agent: AgentId,
  env: EnvId,
  stored: string | undefined,
): { id: string; label: string }[] {
  if (!profilesApply(snapshot, agent, env)) return [];
  const list = agentProfiles(snapshot, agent);
  const shown = list.length > 1 || (!!stored && stored !== DEFAULT_PROFILE);
  return shown ? list.map(({ id, label }) => ({ id, label })) : [];
}

/**
 * The profile a start uses and an edited recent stores, as natively
 * (launch_prep.rs → launch_profile): none for „Standard“ (absent, blank, `default`) and
 * outside Lokal; `null` for an id the agent does not list.
 */
export function editedProfile(
  settings: Pick<AppSettings, 'profiles'>,
  agent: AgentId,
  env: EnvId,
  profile: string | undefined,
): string | undefined | null {
  const id = profile?.trim();
  if (!id || id === DEFAULT_PROFILE || env !== 'local') return undefined;
  return knownProfile(settings, agent, id) ? id : null;
}

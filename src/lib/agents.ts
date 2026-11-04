/*
 * Pure helpers over the snapshot's catalog and probes: labels, icons, the
 * permission cycle, Windows path parts and whether a start can run. The panel
 * renders every agent generically from these; only `EFFORT_NEEDS_MODEL`
 * names one agent.
 */
import type { IconName } from './Icon.svelte';
import type {
  AgentCatalogEntry,
  AgentId,
  AgentStatus,
  Choice,
  EnvId,
  EnvStatus,
  LaunchOptions,
  PermissionChoice,
  PermissionId,
  ProbeState,
  SessionMode,
  Snapshot,
} from './types';

/** Each agent's product mark, for badges, segments and labels. */
export const AGENT_ICONS: Record<AgentId, IconName> = {
  claude: 'claude',
  opencode: 'opencode',
  codex: 'codex',
};

export interface EnvInfo {
  id: EnvId;
  label: string;
  icon: IconName;
}
export const ENVIRONMENTS: readonly EnvInfo[] = [
  { id: 'local', label: 'Lokal', icon: 'monitor' },
  { id: 'docker', label: 'Docker', icon: 'docker' },
  { id: 'wsl', label: 'WSL', icon: 'terminal' },
];
export const envInfo = (env: EnvId): EnvInfo =>
  ENVIRONMENTS.find((item) => item.id === env) ?? ENVIRONMENTS[0]!;

/** Tooltip of the reserved environment (WSL only). */
export const SOON = 'Bald verfügbar';

export const SESSION_LABELS: Record<SessionMode, string> = {
  new: 'Neu',
  continue: 'Fortsetzen',
  resume: 'Auswahl',
};
export const SESSION_HINTS: Record<SessionMode, string> = {
  new: 'Neue Session',
  continue: 'Letzte Session dieses Ordners fortsetzen',
  resume: 'Session aus der Liste des Agenten wählen',
};

export const PERMISSION_ICONS: Record<PermissionId, IconName> = {
  ask: 'shield',
  auto: 'shieldCheck',
  bypass: 'bolt',
};
const PERMISSION_ORDER: readonly PermissionId[] = ['ask', 'auto', 'bypass'];

export function catalogEntry(
  snapshot: Snapshot,
  agent: AgentId,
): AgentCatalogEntry | undefined {
  return snapshot.catalog.find((entry) => entry.id === agent);
}

export function agentLabel(snapshot: Snapshot, agent: AgentId) {
  return catalogEntry(snapshot, agent)?.label ?? agent;
}

export function agentStatus(
  snapshot: Snapshot,
  agent: AgentId,
  env: EnvId,
): AgentStatus | undefined {
  return snapshot.agents.find(
    (status) => status.agent === agent && status.env === env,
  );
}

export function envStatus(
  snapshot: Snapshot,
  env: EnvId,
): EnvStatus | undefined {
  return snapshot.environments.find((status) => status.env === env);
}

/** A local start through an npm `.cmd` shim, which cmd.exe re-parses. */
export function shimStart(snapshot: Snapshot, agent: AgentId, env: EnvId) {
  const path = agentStatus(snapshot, agent, 'local')?.path ?? '';
  return env === 'local' && /\.cmd$/i.test(path);
}

/** The next permission in the cycle ask → auto → bypass → ask. */
export function nextPermission(
  current: PermissionId,
  choices: readonly PermissionChoice[],
): PermissionId {
  const order = choices.length
    ? choices.map((choice) => choice.id)
    : PERMISSION_ORDER;
  const index = order.indexOf(current);
  return order[(index + 1) % order.length] ?? current;
}

/** The catalog's choice, or a neutral stand-in for an unknown id. */
export function permissionChoice(
  entry: AgentCatalogEntry | undefined,
  id: PermissionId,
): PermissionChoice {
  return (
    entry?.permissions.find((choice) => choice.id === id) ?? {
      id,
      label: id,
      hint: '',
      danger: id === 'bypass',
    }
  );
}

/** The label of a model or effort id; free model strings show as typed. */
export function choiceLabel(
  choices: readonly { id: string; label: string }[],
  id: string,
) {
  return choices.find((choice) => choice.id === id)?.label ?? id;
}

/** Tooltip of a model or effort: `Modell: Standard – <catalog hint>`. */
export function choiceTitle(
  name: string,
  choices: readonly Choice[],
  id: string,
) {
  const hint = choices.find((choice) => choice.id === id)?.hint;
  return `${name}: ${choiceLabel(choices, id)}${hint ? ` – ${hint}` : ''}`;
}

/**
 * Agents whose thinking level is a variant of an explicit model (OpenCode
 * builds `provider/model#variant`): with the default model the native side
 * sends no effort. The one agent-specific rule of the panel; every view asks
 * `effortChoices` instead of naming an agent.
 */
const EFFORT_NEEDS_MODEL: ReadonlySet<AgentId> = new Set(['opencode']);

/** The effort choices that apply to `model`; `[]` when none is sent. */
export function effortChoices(
  entry: AgentCatalogEntry | undefined,
  model: string,
): readonly Choice[] {
  if (!entry) return [];
  if (EFFORT_NEEDS_MODEL.has(entry.id) && model === 'default') return [];
  return entry.efforts;
}

/** Whether the catalog lists efforts that the current model cannot use. */
export function effortNeedsModel(
  entry: AgentCatalogEntry | undefined,
  model: string,
) {
  return !!entry?.efforts.length && !effortChoices(entry, model).length;
}

/** The options a start sends and records: an inapplicable effort is `default`. */
export function sentOptions(
  entry: AgentCatalogEntry | undefined,
  options: LaunchOptions,
): LaunchOptions {
  return effortChoices(entry, options.model).length
    ? { ...options }
    : { ...options, effort: 'default' };
}

/** `C:\a\b\` → `C:\a\b`; a drive root keeps its backslash. */
function trimSeparators(path: string) {
  const trimmed = path.trim().replace(/[\\/]+$/, '');
  return /^[A-Za-z]:$/.test(trimmed) ? `${trimmed}\\` : trimmed;
}

/** The last segment of a Windows path: `C:\Users\demo\app` → `app`. */
export function folderName(path: string) {
  const trimmed = trimSeparators(path);
  if (/^[A-Za-z]:\\$/.test(trimmed)) return trimmed;
  return trimmed.split(/[\\/]/).at(-1) || trimmed;
}

/** Everything before the last segment: `C:\Users\demo\app` → `C:\Users\demo`. */
export function parentPath(path: string) {
  const trimmed = trimSeparators(path);
  const cut = Math.max(trimmed.lastIndexOf('\\'), trimmed.lastIndexOf('/'));
  if (cut < 0 || /^[A-Za-z]:\\$/.test(trimmed)) return '';
  const parent = trimmed.slice(0, cut);
  return /^[A-Za-z]:$/.test(parent) ? `${parent}\\` : parent;
}

/** `27.5.1` → `27.5`; other shapes stay as they are. */
export function shortVersion(version: string | undefined) {
  if (!version) return '';
  return /^(\d+\.\d+)\.\d+/.exec(version)?.[1] ?? version;
}

/** German state words for tooltips and status lines. */
export const STATE_LABELS: Record<ProbeState, string> = {
  ready: 'bereit',
  missing: 'nicht installiert',
  stopped: 'nicht erreichbar',
  unavailable: 'nicht verfügbar',
  unknown: 'wird erkannt',
};

/** The Docker line of the footer and the settings. */
export function dockerLine(status: EnvStatus | undefined): {
  text: string;
  state: ProbeState;
} {
  const state = status?.state ?? 'unknown';
  switch (state) {
    case 'ready': {
      const version = shortVersion(status?.version);
      return {
        text: version ? `Docker ${version} · bereit` : 'Docker bereit',
        state,
      };
    }
    case 'missing':
      return { text: 'Docker fehlt', state };
    case 'stopped':
      return { text: 'Docker nicht erreichbar', state };
    case 'unavailable':
      return { text: 'Docker nicht verfügbar', state };
    default:
      return { text: 'Docker wird erkannt …', state };
  }
}

/** Whether a start can run now, and what the primary action becomes. */
export type StartState =
  | { kind: 'ready' }
  /** The CLI is missing locally: the action installs it. */
  | { kind: 'install' }
  | { kind: 'blocked'; reason: string };

/** Probe states in which an environment cannot start anything. */
const DOWN: ReadonlySet<ProbeState> = new Set([
  'missing',
  'stopped',
  'unavailable',
]);

/**
 * Only WSL is „bald“. A Docker that is down or unusable (e.g. Windows
 * containers) stays selectable and names its probe's reason; a local CLI that
 * is missing or cannot start offers the install.
 */
export function startState(
  snapshot: Snapshot,
  target: { agent: AgentId; env: EnvId; folder: string },
): StartState {
  const entry = catalogEntry(snapshot, target.agent);
  const label = entry?.label ?? target.agent;
  const env = envStatus(snapshot, target.env);
  if (target.env === 'wsl') return { kind: 'blocked', reason: SOON };
  if (entry && !entry.environments.includes(target.env))
    return {
      kind: 'blocked',
      reason: `${label} läuft nicht in ${envInfo(target.env).label}`,
    };
  if (target.env === 'local') {
    const local = agentStatus(snapshot, target.agent, 'local')?.state;
    if (local === 'missing' || local === 'unavailable')
      return { kind: 'install' };
  } else if (env && DOWN.has(env.state)) {
    return { kind: 'blocked', reason: env.detail || dockerLine(env).text };
  }
  if (!target.folder.trim())
    return { kind: 'blocked', reason: 'Ordner wählen' };
  return { kind: 'ready' };
}

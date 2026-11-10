/*
 * Synthetic data of the browser preview, labelled `demo: true`. Catalog values
 * follow docs/AGENT_RECIPES.md §4; probes, folders and times are invented and
 * never read from the machine.
 */
import type {
  AgentCatalogEntry,
  AgentStatus,
  AppSettings,
  Choice,
  PermissionChoice,
  PortPreset,
  Snapshot,
} from './types';
import { demoGithub } from './demoGithub';
import { DEMO_HOME, demoRecents } from './demoRecents';

/** `default` (with the catalog's hint) plus the five levels. */
const efforts = (hint: string): Choice[] => [
  { id: 'default', label: 'Standard', hint },
  { id: 'low', label: 'Niedrig' },
  { id: 'medium', label: 'Mittel' },
  { id: 'high', label: 'Hoch' },
  { id: 'xhigh', label: 'Sehr hoch' },
  { id: 'max', label: 'Maximal' },
];

/** The bypass label follows each CLI: „Bypass“ (Claude Code) or „YOLO“. */
const permissions = (bypass: string): PermissionChoice[] => [
  {
    id: 'ask',
    label: 'Nachfragen',
    hint: 'Fragt vor Änderungen und Befehlen',
    danger: false,
  },
  {
    id: 'auto',
    label: 'Auto',
    hint: 'Erlaubt sichere Schritte selbst',
    danger: false,
  },
  {
    id: 'bypass',
    label: bypass,
    hint: 'Überspringt jede Rückfrage',
    danger: true,
  },
];

const models = (
  hint: string,
  ...ids: [id: string, label: string, hint?: string][]
): Choice[] => [
  { id: 'default', label: 'Standard', hint },
  ...ids.map(([id, label, hint]) =>
    hint ? { id, label, hint } : { id, label },
  ),
];

export const DEMO_CATALOG: AgentCatalogEntry[] = [
  {
    id: 'claude',
    label: 'Claude Code',
    models: models(
      'Ohne --model: Standard des Kontos',
      ['fable', 'Fable', 'Für die schwersten, längsten Aufgaben'],
      ['opus', 'Opus'],
      ['sonnet', 'Sonnet'],
      ['haiku', 'Haiku', 'Schnell und günstig'],
      ['opusplan', 'Opus Plan', 'Opus beim Planen, sonst Sonnet'],
    ),
    freeModel: true,
    efforts: efforts('Ohne --effort: Standard des Modells'),
    permissions: permissions('Bypass'),
    sessions: ['new', 'continue', 'resume'],
    environments: ['local', 'docker'],
    install: {
      command: 'irm https://claude.ai/install.ps1 | iex',
      docsUrl: 'https://code.claude.com/docs/en/setup',
      update: 'claude update',
      requires: 'Git for Windows empfohlen',
    },
  },
  {
    id: 'opencode',
    label: 'OpenCode',
    models: models(
      'Aus der OpenCode-Konfiguration; eigenes Modell als provider/model (opencode models)',
    ),
    freeModel: true,
    efforts: efforts('Nur mit gewähltem Modell (Modell-Variante)'),
    permissions: permissions('YOLO'),
    sessions: ['new', 'continue'],
    environments: ['local', 'docker'],
    install: {
      command: 'npm install -g @opencode/cli@latest',
      docsUrl: 'https://opencode.ai/v2/docs/',
      update: 'opencode upgrade',
      requires: 'Node.js und npm',
    },
  },
  {
    id: 'codex',
    label: 'Codex',
    models: models(
      'Ohne -m: Standard aus Konto und Konfiguration',
      ['gpt-6.1-sol', 'gpt-6.1-sol'],
      ['gpt-6-astra', 'gpt-6-astra'],
      ['gpt-6-sol', 'gpt-6-sol'],
      ['gpt-6-luna', 'gpt-6-luna'],
      ['gpt-5.6-sol', 'gpt-5.6-sol'],
      ['gpt-5.5', 'gpt-5.5'],
    ),
    freeModel: true,
    efforts: efforts('Ohne model_reasoning_effort: Standard des Modells'),
    permissions: permissions('YOLO'),
    sessions: ['new', 'continue', 'resume'],
    environments: ['local', 'docker'],
    install: {
      command: 'irm https://chatgpt.com/codex/install.ps1 | iex',
      docsUrl: 'https://github.com/openai/codex',
      update: 'codex update',
      requires: 'Windows 10 1809 oder neuer',
    },
  },
];

const IMAGE = 'node:22-trixie';

/** docs/SPEC.md → Ports: the common dev-server ports, as Rust sends them. */
export const DEMO_PORT_PRESETS: PortPreset[] = [
  { port: 3000, label: 'Next.js · Node' },
  { port: 5173, label: 'Vite' },
  { port: 4200, label: 'Angular' },
  { port: 8000, label: 'Django · FastAPI' },
  { port: 8080, label: 'Spring · HTTP' },
  { port: 5000, label: 'Flask · ASP.NET' },
  { port: 4321, label: 'Astro' },
  { port: 6006, label: 'Storybook' },
];

export function demoSettings(): AppSettings {
  return {
    defaultAgent: 'claude',
    defaultEnv: 'local',
    environments: ['local', 'docker', 'wsl'],
    terminal: 'windowsTerminal',
    sharedWindow: false,
    agents: {
      claude: {
        model: 'default',
        effort: 'default',
        permission: 'auto',
        extraArgs: '',
      },
      opencode: {
        model: 'default',
        effort: 'default',
        permission: 'ask',
        extraArgs: '',
      },
      codex: {
        model: 'default',
        effort: 'default',
        permission: 'ask',
        extraArgs: '',
      },
    },
    docker: {
      images: { claude: IMAGE, opencode: IMAGE, codex: IMAGE },
      ghToken: false,
      ports: [],
    },
    github: { cloneRoot: '' },
    agentUpdates: false,
  };
}

const DEMO_AGENTS: AgentStatus[] = [
  {
    agent: 'claude',
    env: 'local',
    state: 'ready',
    version: '2.1.317',
    path: `${DEMO_HOME}\\.local\\bin\\claude.exe`,
  },
  { agent: 'opencode', env: 'local', state: 'missing' },
  {
    agent: 'codex',
    env: 'local',
    state: 'ready',
    version: '0.166.0',
    path: `${DEMO_HOME}\\AppData\\Local\\Programs\\OpenAI\\Codex\\bin\\codex.exe`,
  },
  { agent: 'claude', env: 'docker', state: 'ready' },
  {
    agent: 'opencode',
    env: 'docker',
    state: 'ready',
    detail: 'CLI wird beim ersten Start installiert',
  },
  {
    agent: 'codex',
    env: 'docker',
    state: 'ready',
    detail: 'Image wird beim ersten Start geladen',
  },
];

/**
 * The browser preview's snapshot: simulated, labelled `demo`, never read from
 * the machine. It starts as if the startup detection had just finished.
 */
export function demoSnapshot(now: number = Date.now()): Snapshot {
  return {
    demo: true,
    settings: demoSettings(),
    catalog: structuredClone(DEMO_CATALOG),
    portPresets: structuredClone(DEMO_PORT_PRESETS),
    agents: structuredClone(DEMO_AGENTS),
    environments: [
      { env: 'local', state: 'ready' },
      { env: 'docker', state: 'ready', version: '27.5.1' },
      { env: 'wsl', state: 'unavailable', detail: 'Bald verfügbar' },
    ],
    recents: demoRecents(now, demoSettings().agents),
    github: demoGithub(now),
    detecting: false,
    agentUpdates: [],
    revision: 1,
    checkedAt: new Date(now).toISOString(),
  };
}

/** The folder the demo's folder dialog „picks“. */
export const DEMO_PICKED_FOLDER = `${DEMO_HOME}\\Projekte\\demo-app`;

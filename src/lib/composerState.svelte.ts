import { catalogEntry, nextPermission, sentOptions } from './agents';
import { shownEnv, shownEnvs } from './environments';
import {
  DEFAULT_PROFILE,
  agentProfiles,
  defaultProfile,
  knownProfile,
  profilesApply,
} from './profiles';
import { classifyTarget, repoLabel, type Target } from './target';
import type {
  AgentId,
  AppSettings,
  EnvId,
  LaunchOptions,
  LaunchRequest,
  RecentSession,
  SessionMode,
  Snapshot,
} from './types';

/** Before the first snapshot: the native default options. */
const FALLBACK: LaunchOptions = {
  model: 'default',
  effort: 'default',
  permission: 'auto',
  extraArgs: '',
};

const same = (a: unknown, b: unknown) =>
  JSON.stringify(a) === JSON.stringify(b);

/** The field text that starts a recent again: its repository, else its folder. */
export const recentTarget = (recent: RecentSession | undefined) =>
  recent ? (recent.repo ? repoLabel(recent.repo) : recent.folder) : '';

/**
 * The „Neue Session“ form. It starts from the saved defaults (agent,
 * environment, the agent's options) and the newest recent target; what the
 * user changes here applies to the next start only and never to the defaults.
 * Its environment is always one the settings show (`environments.ts`).
 */
export class Composer {
  agent = $state<AgentId>('claude');
  env = $state<EnvId>('local');
  /** The field text: a folder, a repository or search text (`target`). */
  folder = $state('');
  options = $state<LaunchOptions>({ ...FALLBACK });
  /** Per start only; back to `new` after every start and agent change. */
  session = $state<SessionMode>('new');
  /**
   * Published ports, sent with a Docker start only; from the saved Docker
   * defaults, back to them after every start.
   */
  ports = $state<string[]>([]);
  /**
   * The agent's profile (Lokal only); from its saved default, back to it
   * after every start and agent change.
   */
  profile = $state(DEFAULT_PROFILE);
  /** The snapshot the form follows; null until the first one. */
  #snapshot = $state.raw<Snapshot | null>(null);

  get ready() {
    return this.#snapshot !== null;
  }

  /**
   * Follows the snapshot: the first one initialises the form; a later one
   * applies only defaults that changed (a saved settings dialog) and keeps
   * every other choice.
   */
  sync(snapshot: Snapshot) {
    const previous = this.#snapshot?.settings;
    this.#snapshot = snapshot;
    const settings = snapshot.settings;
    if (!previous) {
      this.agent = settings.defaultAgent;
      this.env = shownEnv(settings.defaultEnv, settings.environments);
      this.resetOptions();
      this.resetPorts();
      return;
    }
    if (settings.defaultAgent !== previous.defaultAgent)
      this.agent = settings.defaultAgent;
    if (settings.defaultEnv !== previous.defaultEnv)
      this.env = settings.defaultEnv;
    // A saved settings dialog may have hidden the selected environment.
    this.env = shownEnv(this.env, settings.environments);
    if (
      settings.defaultAgent !== previous.defaultAgent ||
      !same(settings.agents[this.agent], previous.agents[this.agent])
    )
      this.resetOptions();
    if (!same(settings.docker.ports, previous.docker.ports)) this.resetPorts();
    // A changed default, or the chosen profile removed meanwhile.
    if (
      settings.agentProfile?.[this.agent] !==
        previous.agentProfile?.[this.agent] ||
      !knownProfile(settings, this.agent, this.profile)
    )
      this.profile = defaultProfile(settings, this.agent);
  }

  /** The environments the form offers, in display order. */
  get environments() {
    return shownEnvs(this.#snapshot?.settings.environments);
  }

  /** What the field text names (docs/SPEC.md → Repositories). */
  get target(): Target {
    return classifyTarget(this.folder);
  }

  #defaults(settings: AppSettings | undefined): LaunchOptions {
    return { ...(settings?.agents[this.agent] ?? FALLBACK) };
  }

  /** The agent's saved defaults (its profile too) and a new session. */
  resetOptions() {
    const settings = this.#snapshot?.settings;
    this.options = this.#defaults(settings);
    this.profile = settings
      ? defaultProfile(settings, this.agent)
      : DEFAULT_PROFILE;
    this.session = 'new';
  }

  /** The saved Docker default ports. */
  resetPorts() {
    this.ports = [...(this.#snapshot?.settings.docker.ports ?? [])];
  }

  selectAgent(agent: AgentId) {
    if (agent === this.agent) return;
    this.agent = agent;
    this.resetOptions();
  }

  selectEnv(env: EnvId) {
    this.env = env;
  }

  cyclePermission() {
    const entry = this.#snapshot && catalogEntry(this.#snapshot, this.agent);
    this.options.permission = nextPermission(
      this.options.permission,
      entry?.permissions ?? [],
    );
  }

  /**
   * „In Composer übernehmen“: that session's target, options and ports; a
   * hidden environment becomes the first shown one.
   */
  adopt(recent: RecentSession) {
    this.agent = recent.agent;
    this.env = shownEnv(recent.env, this.#snapshot?.settings.environments);
    this.folder = recentTarget(recent);
    this.options = { ...recent.options };
    const settings = this.#snapshot?.settings;
    const profile = recent.profile ?? DEFAULT_PROFILE;
    this.profile =
      !settings || knownProfile(settings, recent.agent, profile)
        ? profile
        : defaultProfile(settings, recent.agent);
    this.session = 'new';
    // A Lokal or WSL session published none; Docker then starts from the defaults.
    if (recent.env === 'docker' && this.env === 'docker')
      this.ports = [...recent.ports];
    else this.resetPorts();
  }

  /** Whether a start sends the profile: an agent with profiles, Lokal. */
  get profileApplies() {
    return (
      !!this.#snapshot && profilesApply(this.#snapshot, this.agent, this.env)
    );
  }

  /**
   * The profiles the „Profil“ chip offers, „Standard“ first; empty (no chip)
   * where the profile does not apply or the agent has no extra profile.
   */
  get profiles() {
    const snapshot = this.#snapshot;
    if (!snapshot || !this.profileApplies) return [];
    const list = agentProfiles(snapshot, this.agent);
    return list.length > 1 ? list : [];
  }

  /** The session modes the selected agent supports; `new` always. */
  get sessions(): SessionMode[] {
    const entry = this.#snapshot && catalogEntry(this.#snapshot, this.agent);
    return entry?.sessions.length ? entry.sessions : ['new'];
  }

  /**
   * What „Starten“ sends: a repository with folder `''`, else the trimmed
   * field text; an effort the model cannot use goes as `default`.
   */
  request(): LaunchRequest {
    const session = this.sessions.includes(this.session) ? this.session : 'new';
    const entry = this.#snapshot
      ? catalogEntry(this.#snapshot, this.agent)
      : undefined;
    const target = this.target;
    return {
      agent: this.agent,
      env: this.env,
      folder: target.kind === 'repo' ? '' : this.folder.trim(),
      ...(target.kind === 'repo' && { repo: target.repo }),
      options: sentOptions(entry, $state.snapshot(this.options)),
      session,
      ports: this.env === 'docker' ? $state.snapshot(this.ports) : [],
      ...(this.profileApplies &&
        this.profile !== DEFAULT_PROFILE && { profile: this.profile }),
    };
  }

  /** After a successful start: the next one begins from the defaults again. */
  launched() {
    this.resetOptions();
    this.resetPorts();
  }
}

import { agentLabel, agentStatus, effortProblem, envStatus } from './agents';
import type { Bridge, ObserveSnapshots } from './bridgeTypes';
import {
  agentUpdatesProblem,
  agentUpdateSteps,
  launchUpdateProblem,
  scheduleSteps,
} from './demoAgentUpdates';
import { DEMO_PICKED_FOLDER, demoSnapshot } from './demoData';
import {
  DEMO_ACCOUNT,
  DEMO_LOGIN,
  demoLocalAccounts,
  demoRepos,
} from './demoGithub';
import { demoProfiles, signedIn, syncedProfiles } from './demoProfiles';
import {
  PROFILE_MISSING,
  editedProfile,
  launchProfileProblem,
} from './profiles';
import {
  editRecent,
  recordStart,
  restoreRecent,
  upsertRecent,
  withoutRecent,
} from './recents';
import { cloneFolder, repoProblem } from './target';
import type { GithubAccount, GithubSource, Snapshot } from './types';
import {
  folderProblem,
  launchPortsProblem,
  optionsProblem,
  settingsProblems,
} from './validation';
import { normalizeEnvs } from './environments';
import { normalizePorts } from './ports';

export type { Bridge } from './bridgeTypes';
export { demoSnapshot } from './demoData';

/** The demo bridge and the snapshots its simulated detection pushes. */
export interface DemoBridge extends Bridge {
  observe: ObserveSnapshots;
}

/** What the demo accepts as a personal access token. */
const DEMO_TOKEN = /^(?:gh[pousr]_|github_pat_)\w{4,}$/;

/**
 * A local simulation of the native commands, with a short latency: starts
 * record recent sessions, a detection finishes after `detection` ms and is
 * pushed like the native `traygent:snapshot` event; an agent update run
 * pushes each of its steps, one every `detection / 2` ms. Every change
 * increases `revision`, as natively. No terminal, dialog
 * or Explorer window ever opens; every error is German, as natively.
 */
export function createDemoBridge(latency = 150, detection = 600): DemoBridge {
  let data = demoSnapshot();
  const listeners = new Set<(snapshot: Snapshot) => void>();
  let detectionTimer: ReturnType<typeof setTimeout> | undefined;
  let listingTimer: ReturnType<typeof setTimeout> | undefined;
  const delay = () =>
    new Promise<void>((resolve) => setTimeout(resolve, latency));
  const copy = () => structuredClone(data);
  /** Applies one state change under the next revision. */
  const change = (patch: Partial<Snapshot>) => {
    data = { ...data, ...patch, revision: data.revision + 1 };
  };
  const emit = () => {
    for (const listener of listeners) listener(copy());
  };
  /** The account's listing, just read: no error, not loading. */
  const listed = (account: GithubAccount) => {
    change({
      github: {
        account,
        repos: demoRepos(),
        loading: false,
        listedAt: new Date().toISOString(),
        defaultCloneRoot: data.github.defaultCloneRoot,
      },
    });
  };
  /** Connects the demo account and lists its repositories at once. */
  const connect = (source: GithubSource) => {
    clearTimeout(listingTimer);
    listed({ ...DEMO_ACCOUNT, source });
    return copy();
  };

  return {
    async snapshot() {
      await delay();
      return copy();
    },
    async refreshDetection() {
      await delay();
      change({ detecting: true });
      clearTimeout(detectionTimer);
      detectionTimer = setTimeout(() => {
        change({ detecting: false, checkedAt: new Date().toISOString() });
        emit();
      }, detection);
      return copy();
    },
    async saveSettings(settings) {
      await delay();
      const labels = Object.fromEntries(
        data.catalog.map((item) => [item.id, item.label]),
      );
      const problems = settingsProblems(settings, labels);
      const root = problems['github.cloneRoot'];
      // Natively the clone root's message names its field.
      const problem = Object.values(problems)[0];
      if (problem)
        throw new Error(problem === root ? `Klon-Ordner: ${root}` : problem);
      const saved = structuredClone(settings);
      saved.docker.ports = normalizePorts(saved.docker.ports);
      saved.environments = normalizeEnvs(saved.environments);
      const profiles = demoProfiles(saved, data.catalog, data.profiles);
      change({ settings: saved, profiles });
      return copy();
    },
    async launchSession(request) {
      await delay();
      const label = agentLabel(data, request.agent);
      const env = envStatus(data, request.env);
      if (request.env === 'wsl' || env?.state !== 'ready')
        throw new Error('Diese Umgebung ist nicht bereit.');
      const updating = launchUpdateProblem(data, request);
      if (updating) throw new Error(updating);
      if (
        request.env === 'local' &&
        agentStatus(data, request.agent, 'local')?.state !== 'ready'
      )
        throw new Error(`${label} ist lokal nicht installiert.`);
      const { repo } = request;
      const given = request.folder.trim();
      const problem =
        (repo ? repoProblem(repo) : '') ||
        (repo && !given ? '' : folderProblem(request.folder)) ||
        optionsProblem(request) ||
        launchPortsProblem(request) ||
        launchProfileProblem(data, request);
      if (problem) throw new Error(problem);
      // Every demo folder exists: a repository start reuses the folder it
      // brings (a recent's restart), else starts in its clone folder, as
      // natively; nothing clones.
      const root =
        data.settings.github.cloneRoot.trim() || data.github.defaultCloneRoot;
      const folder = repo && !given ? cloneFolder(root, repo) : request.folder;
      // Normalized like natively: the recent stores what the start published
      // (no profile outside Lokal or for „Standard“).
      const ports = normalizePorts(request.ports);
      const { profile: wanted, ...rest } = request;
      const profile = editedProfile(
        data.settings,
        rest.agent,
        rest.env,
        wanted,
      );
      const recent = recordStart(data.recents, {
        ...rest,
        folder,
        ports,
        ...(profile && { profile }),
      });
      change({ recents: upsertRecent(data.recents, recent) });
      return { recent: structuredClone(recent), snapshot: copy() };
    },
    async removeRecent(id) {
      await delay();
      change({ recents: withoutRecent(data.recents, id) });
      return copy();
    },
    async restoreRecent(recent) {
      await delay();
      const restored = restoreRecent(data.recents, structuredClone(recent));
      if (!restored)
        throw new Error('Eintrag kann nicht wiederhergestellt werden.');
      change({ recents: restored });
      return copy();
    },
    async updateRecent(id, options, ports, profile) {
      await delay();
      const recent = data.recents.find((item) => item.id === id);
      if (!recent)
        throw new Error('Diese Session steht nicht mehr in „Zuletzt“.');
      const entry = data.catalog.find((item) => item.id === recent.agent);
      const problem =
        optionsProblem({ agent: recent.agent, options }) ||
        effortProblem(entry, options) ||
        launchPortsProblem({ env: recent.env, ports });
      if (problem) throw new Error(problem);
      const { agent, env } = recent;
      const stored = editedProfile(data.settings, agent, env, profile);
      if (stored === null) throw new Error(PROFILE_MISSING);
      const edited = editRecent(
        data.recents,
        id,
        structuredClone(options),
        normalizePorts(ports),
        stored,
      );
      if (edited) change({ recents: edited });
      return copy();
    },
    async pickFolder() {
      await delay();
      return DEMO_PICKED_FOLDER;
    },
    async installAgent() {
      await delay();
    },
    async openFolder() {
      await delay();
    },
    async githubConnectToken(token) {
      await delay();
      const value = token.trim();
      if (!value) throw new Error('Token fehlt.');
      if (!DEMO_TOKEN.test(value))
        throw new Error('GitHub hat das Token abgelehnt.');
      return connect('token');
    },
    async githubLocalAccounts() {
      await delay();
      return demoLocalAccounts(data.github.account?.login);
    },
    async githubImportLocal(login) {
      await delay();
      if (login !== DEMO_LOGIN)
        throw new Error(`Die Anmeldung ${login} lässt sich nicht übernehmen.`);
      return connect('gh');
    },
    async githubDisconnect() {
      await delay();
      clearTimeout(listingTimer);
      const { defaultCloneRoot } = data.github;
      change({ github: { repos: [], loading: false, defaultCloneRoot } });
      return copy();
    },
    async githubRefreshRepos() {
      await delay();
      if (!data.github.account) throw new Error('Kein GitHub-Konto verbunden.');
      change({ github: { ...data.github, loading: true } });
      clearTimeout(listingTimer);
      listingTimer = setTimeout(() => {
        if (!data.github.account) return;
        listed(data.github.account);
        emit();
      }, detection);
      return copy();
    },
    async updateAgents() {
      await delay();
      const problem = agentUpdatesProblem(data);
      if (problem) throw new Error(problem);
      const steps = agentUpdateSteps(data);
      change({ agentUpdatesRunning: true });
      const apply = (step: (typeof steps)[number]) => {
        change(step(data));
        emit();
      };
      scheduleSteps(steps, apply, detection / 2);
      return copy();
    },
    async syncProfiles() {
      await delay();
      change({ profiles: syncedProfiles(data.profiles, Date.now()) });
      return copy();
    },
    async profileLogin(agent, profile) {
      await delay();
      if (!data.profiles.some((p) => p.agent === agent && p.id === profile))
        throw new Error('Dieses Profil gibt es nicht.');
      // No terminal opens; the sign-in „finishes“ like a detection.
      setTimeout(() => {
        change({ profiles: signedIn(data.profiles, agent, profile) });
        emit();
      }, detection);
    },
    async observe(callback) {
      listeners.add(callback);
      return () => listeners.delete(callback);
    },
  };
}

import { toggleEnv } from './environments';
import { errorMessage } from './format';
import { emptyShare, type ShareKind } from './profileShare';
import { DEFAULT_PROFILE, newProfileProblem, profileId } from './profiles';
import type { AgentId, AgentProfile, AppSettings, EnvId } from './types';
import { settingsProblems, type SettingsField } from './validation';

const same = (a: unknown, b: unknown) =>
  JSON.stringify(a) === JSON.stringify(b);
const plain = (value: unknown): value is Record<string, unknown> =>
  !!value && typeof value === 'object' && !Array.isArray(value);

/**
 * `value` moved from the baseline `from` onto `to`: whatever still equals
 * `from` takes `to`'s value, a staged edit stays. Objects merge key by key,
 * lists and scalars are one field each.
 */
function rebase<T>(from: T, to: T, value: T): T {
  if (same(value, from)) return structuredClone(to);
  if (!plain(from) || !plain(to) || !plain(value)) return value;
  const merged: Record<string, unknown> = {};
  for (const key of new Set([...Object.keys(to), ...Object.keys(value)])) {
    const next = rebase(from[key], to[key], value[key]);
    if (next !== undefined) merged[key] = next;
  }
  return merged as T;
}

/** Saves settings at once (`save_settings`); rejects with the German message. */
export type SaveNow = (settings: AppSettings) => Promise<AppSettings>;

/**
 * The staged settings of the „Agenten“, „Umgebung“, „GitHub“ and „Updates“
 * tabs. The fields edit `value`; „Speichern“ sends it whole (`save_settings`)
 * and commits the answer as the new baseline, „Abbrechen“ drops it with the
 * dialog. Adding a profile is the one immediate change (`addProfile`); it
 * moves the baseline under the staged edits. `labels` name the agents in the
 * messages that name one.
 */
export class SettingsDraft {
  /** The baseline: the settings as last saved. */
  saved: AppSettings;
  value: AppSettings;
  readonly problems: Partial<Record<SettingsField, string>>;
  readonly dirty: boolean;
  readonly invalid: boolean;
  /** A new profile is being saved at once (`addProfile`). */
  adding = $state(false);
  /** „Speichern“ runs; set by the dialog. */
  saving = $state(false);

  constructor(
    settings: AppSettings,
    labels: Partial<Record<AgentId, string>> = {},
  ) {
    this.saved = $state.raw(settings);
    this.value = $state(structuredClone(settings));
    this.problems = $derived(settingsProblems(this.value, labels));
    this.dirty = $derived(!same($state.snapshot(this.value), this.saved));
    this.invalid = $derived(Object.keys(this.problems).length > 0);
  }

  problem(field: SettingsField) {
    return this.problems[field] ?? '';
  }

  /** Whether a field of this group is invalid, e.g. `docker`, or the field itself. */
  invalidIn(group: 'agents' | 'docker' | 'environments' | 'github') {
    return Object.keys(this.problems).some(
      (field) => field === group || field.startsWith(`${group}.`),
    );
  }

  /** Docker's settings and probes are in view while the staged list shows Docker. */
  get dockerInView() {
    return this.value.environments.includes('docker');
  }

  /**
   * Shows or hides one environment. Hiding Docker resets each invalid Docker
   * field to its saved value and keeps the valid edits staged, so a hidden
   * field never blocks „Speichern“ and nothing half-typed is saved. The
   * default ports are one field: a list with a conflict resets as a whole.
   */
  toggleEnv(env: EnvId, on: boolean) {
    toggleEnv(this.value, env, on);
    if (env !== 'docker' || on) return;
    const docker = this.value.docker;
    for (const field of Object.keys(this.problems) as SettingsField[]) {
      if (field === 'docker.ports') docker.ports = [...this.saved.docker.ports];
      else if (field.startsWith('docker.images.')) {
        const agent = field.slice('docker.images.'.length) as AgentId;
        docker.images[agent] = this.saved.docker.images[agent];
      }
    }
  }

  /** An add or „Speichern“ runs: the other one waits. */
  get busy() {
    return this.adding || this.saving;
  }

  /**
   * Adds one more profile of `agent` from a typed name at once (#37), its id
   * derived (`profiles.ts → profileId`): `save` gets the saved baseline plus
   * the new profile, never the other staged edits, and the answer becomes
   * the baseline (`adopt`). A name whose id the baseline still holds (a
   * staged removal) brings that profile back instead, without a save.
   * Resolves to why it was refused, `''` when added, null while busy.
   */
  async addProfile(
    agent: AgentId,
    name: string,
    save: SaveNow,
  ): Promise<string | null> {
    if (this.busy) return null;
    const label = name.trim();
    const problem = newProfileProblem(label, this.value.profiles[agent] ?? []);
    if (problem) return problem;
    const profile = { id: profileId(label), label };
    const stored = this.saved.profiles?.[agent] ?? [];
    if (stored.some((item) => item.id === profile.id)) {
      this.#restore(agent, profile);
      return '';
    }
    // Only a staged removal or rename hides a clash with the baseline.
    const clash = newProfileProblem(label, stored);
    if (clash) return `${clash} Erst speichern.`;
    const base = structuredClone(this.saved);
    base.profiles = { ...base.profiles, [agent]: [...stored, profile] };
    this.adding = true;
    try {
      this.adopt(await save(base), agent, profile.id);
      return '';
    } catch (cause) {
      return errorMessage(cause);
    } finally {
      this.adding = false;
    }
  }

  /**
   * Takes the answer of an immediate save as the new baseline and keeps
   * every staged edit staged (`rebase`); the profile `id` of `agent` the
   * answer added joins the staged list, so the add alone never makes the
   * draft dirty.
   */
  adopt(settings: AppSettings, agent: AgentId, id: string) {
    const value = rebase(this.saved, settings, $state.snapshot(this.value));
    const added = settings.profiles?.[agent]?.find((item) => item.id === id);
    const list = (value.profiles[agent] ??= []);
    if (added && !list.some((item) => item.id === id)) list.push({ ...added });
    this.saved = settings;
    this.value = value;
  }

  /** Undoes a staged removal: back at its saved place, with the typed name. */
  #restore(agent: AgentId, profile: AgentProfile) {
    const order = (this.saved.profiles?.[agent] ?? []).map((item) => item.id);
    const place = (item: AgentProfile) => {
      const index = order.indexOf(item.id);
      return index < 0 ? order.length : index;
    };
    const list = [...(this.value.profiles[agent] ?? []), profile];
    this.value.profiles[agent] = list.sort((a, b) => place(a) - place(b));
  }

  /**
   * Stages a removal; a removed default profile falls back to „Standard“.
   * With the last one gone the shared names are hidden: valid edits stay
   * staged, an invalid list goes back to its saved value, so a hidden field
   * never blocks „Speichern“ (like hiding Docker).
   */
  removeProfile(agent: AgentId, id: string) {
    const list = this.value.profiles[agent] ?? [];
    this.value.profiles[agent] = list.filter((profile) => profile.id !== id);
    if (this.value.agentProfile[agent] === id)
      this.value.agentProfile[agent] = DEFAULT_PROFILE;
    if (this.value.profiles[agent].length) return;
    for (const kind of ['dirs', 'files'] as const)
      if (this.problem(`agents.${agent}.share.${kind}`))
        this.setShare(agent, kind, [
          ...(this.saved.profileShare?.[agent]?.[kind] ?? []),
        ]);
  }

  /** Stages the extra shared folders or files of `agent`. */
  setShare(agent: AgentId, kind: ShareKind, names: string[]) {
    this.value.profileShare ??= {} as AppSettings['profileShare'];
    (this.value.profileShare[agent] ??= emptyShare())[kind] = names;
  }

  /** A plain copy for `save_settings`. */
  current(): AppSettings {
    return $state.snapshot(this.value);
  }

  commit(settings: AppSettings) {
    this.saved = settings;
    this.value = structuredClone(settings);
  }
}

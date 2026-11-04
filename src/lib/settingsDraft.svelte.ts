import { toggleEnv } from './environments';
import type { AgentId, AppSettings, EnvId } from './types';
import { settingsProblems, type SettingsField } from './validation';

const same = (a: unknown, b: unknown) =>
  JSON.stringify(a) === JSON.stringify(b);

/**
 * The staged settings of the „Agenten“, „Umgebung“ and „GitHub“ tabs. The fields edit
 * `value`; „Speichern“ sends it whole (`save_settings`) and commits the
 * answer as the new baseline, „Abbrechen“ drops it with the dialog.
 */
export class SettingsDraft {
  /** The baseline: the settings as last saved. */
  saved: AppSettings;
  value: AppSettings;
  readonly problems: Partial<Record<SettingsField, string>>;
  readonly dirty: boolean;
  readonly invalid: boolean;

  constructor(settings: AppSettings) {
    this.saved = $state.raw(settings);
    this.value = $state(structuredClone(settings));
    this.problems = $derived(settingsProblems(this.value));
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

  /** A plain copy for `save_settings`. */
  current(): AppSettings {
    return $state.snapshot(this.value);
  }

  commit(settings: AppSettings) {
    this.saved = settings;
    this.value = structuredClone(settings);
  }
}

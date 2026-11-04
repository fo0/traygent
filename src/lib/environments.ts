/*
 * Which environments the composer offers (`settings.environments`), mirroring
 * src-tauri/src/settings.rs: each once in canonical order, at least one that
 * starts a session (Lokal or Docker), the default among them. Hiding one is a
 * display preference: recents keep their environment and restart unchanged.
 */
import { ENVIRONMENTS } from './agents';
import type { AppSettings, EnvId } from './types';

const ORDER: readonly EnvId[] = ENVIRONMENTS.map((env) => env.id);
export const NO_ENVIRONMENT = 'Mindestens eine Umgebung muss aktiv sein.';

/** Lokal and Docker start sessions; WSL is listed but not available yet. */
export const canStart = (env: EnvId) => env !== 'wsl';

/** Each environment once, in canonical order. */
export const normalizeEnvs = (list: readonly EnvId[]): EnvId[] =>
  ORDER.filter((env) => list.includes(env));

/** What the composer offers; a list without a usable environment offers all. */
export function shownEnvs(list: readonly EnvId[] | undefined): EnvId[] {
  const shown = normalizeEnvs(list ?? ORDER);
  return shown.some(canStart) ? shown : [...ORDER];
}

/** `env` while it is shown, else the first shown one that starts sessions. */
export function shownEnv(env: EnvId, list: readonly EnvId[] | undefined) {
  const shown = shownEnvs(list);
  return shown.includes(env) ? env : (shown.find(canStart) ?? 'local');
}

/** Turning `env` off would leave no environment that starts a session. */
export const lastEnv = (list: readonly EnvId[], env: EnvId) =>
  canStart(env) &&
  list.includes(env) &&
  !list.some((other) => other !== env && canStart(other));

/**
 * Shows or hides one environment in the staged settings; a hidden default
 * moves to the first shown environment that starts sessions.
 */
export function toggleEnv(settings: AppSettings, env: EnvId, on: boolean) {
  const rest = settings.environments.filter((item) => item !== env);
  settings.environments = normalizeEnvs(on ? [...rest, env] : rest);
  if (!settings.environments.includes(settings.defaultEnv))
    settings.defaultEnv =
      settings.environments.find(canStart) ?? settings.defaultEnv;
}

/** Why the native side would refuse the list; empty when it takes it. */
export function envsProblem(
  settings: Pick<AppSettings, 'environments' | 'defaultEnv'>,
) {
  const list = settings.environments;
  if (!list.length) return NO_ENVIRONMENT;
  if (!list.some(canStart)) return 'WSL ist noch nicht verfügbar.';
  if (!list.includes(settings.defaultEnv))
    return 'Die Standard-Umgebung ist deaktiviert.';
  return '';
}

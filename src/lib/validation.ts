/*
 * Early, panel-side checks of what the user types. They mirror the native
 * rules in src-tauri/src/settings.rs, which validates again and has the last
 * word (docs/SPEC.md → Launch rules); these only keep an obviously invalid
 * value from reaching it.
 */
import { envsProblem } from './environments';
import { DOCKER_ONLY, portListProblem } from './ports';
import { DEFAULT_PROFILE, profilesProblem } from './profiles';
import type { AgentId, AppSettings, LaunchRequest } from './types';

export { portListProblem } from './ports';

/** Unicode control characters (Rust `char::is_control`: C0, DEL and C1). */
// eslint-disable-next-line no-control-regex
const CONTROL = /[\u0000-\u001f\u007f-\u009f]/;
/** ASCII quotes, the backtick and the typographic quotes PowerShell also reads as quotes. */
const QUOTES = /["'`‘’‚‛“”„]/;
/**
 * What cmd.exe re-parses when an npm `.cmd` shim forwards its arguments; `(`
 * `)` close the `IF EXIST … ( … %* )` block the shim forwards them in.
 */
const SHIM_UNSAFE = /[&|<>^%!()]/;
const MAX_MODEL_CHARS = 128;
const MAX_EXTRA_ARGS_CHARS = 1000;
const MAX_IMAGE_CHARS = 255;
/** Model ids: ASCII letters, digits and `._:/@[]-`; `#` is built from the effort. */
const MODEL = /^[A-Za-z0-9._:/@[\]-]+$/;
/** OpenCode's `provider/model` form (`^[\w.-]+/[\w.:@/-]+$`). */
const PROVIDER_MODEL = /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.:@/-]+$/;
/** Agents whose model must name its provider. */
const NEEDS_PROVIDER: ReadonlySet<AgentId> = new Set(['opencode']);
/** Docker references: ASCII letters, digits and `._/:@-`, starting alphanumeric. */
const IMAGE = /^[A-Za-z0-9][A-Za-z0-9._/:@-]*$/;

/** Length in Unicode scalar values, as Rust counts `chars()`. */
const chars = (text: string) => [...text].length;

/** Extra CLI arguments: split on whitespace, no quotes, no control characters. */
export function argsProblem(text: string) {
  const value = text.trim();
  if (chars(value) > MAX_EXTRA_ARGS_CHARS) return 'Zu lang.';
  if (CONTROL.test(value)) return 'Keine Steuerzeichen.';
  if (QUOTES.test(value)) return 'Keine Anführungszeichen.';
  return '';
}

/** Extra arguments for a CLI found as an npm `.cmd` shim, which cmd.exe re-parses. */
export function shimArgsProblem(text: string) {
  return SHIM_UNSAFE.test(text)
    ? 'Bei einer npm-Installation keines der Zeichen & | < > ^ % ! ( )'
    : '';
}

/** A model id, or `provider/model` where the agent needs it; empty is the default. */
export function modelProblem(text: string, agent?: AgentId) {
  const value = text.trim();
  if (!value || value === 'default') return '';
  if (chars(value) > MAX_MODEL_CHARS) return 'Modell zu lang.';
  // A leading `-` would turn the value into a flag of its own.
  if (value.startsWith('-') || !MODEL.test(value)) return 'Ungültiges Modell.';
  if (agent && NEEDS_PROVIDER.has(agent) && !PROVIDER_MODEL.test(value))
    return 'Modell als provider/model angeben.';
  return '';
}

/** A Docker image reference such as `node:22-trixie`. */
export function imageProblem(text: string) {
  const value = text.trim();
  if (!value) return 'Image fehlt.';
  if (value.length > MAX_IMAGE_CHARS || !IMAGE.test(value))
    return 'Ungültiges Image.';
  return '';
}

/**
 * `C:\…` / `C:/…` or a UNC path (`\\…` / `//…` and more), as Rust's
 * `launch::is_windows_absolute` decides it.
 */
export const WINDOWS_ABSOLUTE = /^(?:[A-Za-z]:[\\/]|(?:\\\\|\/\/)[\s\S])/;

/** A Windows folder the native core can check: absolute drive or UNC path. */
export function folderProblem(text: string) {
  const value = text.trim();
  if (!value) return 'Ordner fehlt.';
  if (CONTROL.test(value)) return 'Keine Steuerzeichen.';
  if (!WINDOWS_ABSOLUTE.test(value))
    return 'Ordner muss ein absoluter Windows-Pfad sein.';
  return '';
}

/**
 * Why a start would be refused for its options; `shim` when the local CLI is
 * an npm `.cmd` shim. Empty when the native side may take it.
 */
export function optionsProblem(
  request: Pick<LaunchRequest, 'agent' | 'options'>,
  shim = false,
) {
  const { agent, options } = request;
  const args =
    argsProblem(options.extraArgs) ||
    (shim ? shimArgsProblem(options.extraArgs) : '');
  return (
    modelProblem(options.model, agent) ||
    (args ? `Extra-Argumente: ${args}` : '')
  );
}

/** The published ports of a start, checked in the native order: Docker only. */
export function launchPortsProblem(
  request: Pick<LaunchRequest, 'env' | 'ports'>,
) {
  const problem = portListProblem(request.ports);
  if (problem) return problem;
  const any = request.ports.some((entry) => entry.trim());
  return request.env !== 'docker' && any ? DOCKER_ONLY : '';
}

/** Field keys of the settings form, e.g. `agents.claude.extraArgs`. */
export type SettingsField =
  | `agents.${AgentId}.extraArgs`
  | `agents.${AgentId}.model`
  | `agents.${AgentId}.profiles`
  | `docker.images.${AgentId}`
  | 'docker.ports'
  | 'environments'
  | 'github.cloneRoot';

/** Every invalid field of the settings, with its German message. */
export function settingsProblems(
  settings: AppSettings,
): Partial<Record<SettingsField, string>> {
  const problems: Partial<Record<SettingsField, string>> = {};
  const environments = envsProblem(settings);
  if (environments) problems.environments = environments;
  for (const agent of Object.keys(settings.agents) as AgentId[]) {
    const options = settings.agents[agent];
    const args = argsProblem(options.extraArgs);
    if (args) problems[`agents.${agent}.extraArgs`] = args;
    const model = modelProblem(options.model, agent);
    if (model) problems[`agents.${agent}.model`] = model;
    const chosen = settings.agentProfile[agent] ?? DEFAULT_PROFILE;
    const profiles = profilesProblem(settings.profiles[agent] ?? [], chosen);
    if (profiles) problems[`agents.${agent}.profiles`] = profiles;
  }
  for (const agent of Object.keys(settings.docker.images) as AgentId[]) {
    const image = imageProblem(settings.docker.images[agent]);
    if (image) problems[`docker.images.${agent}`] = image;
  }
  const ports = portListProblem(settings.docker.ports);
  if (ports) problems['docker.ports'] = ports;
  // Empty is the default clone root; anything else is a folder.
  const cloneRoot = settings.github.cloneRoot.trim();
  const root = cloneRoot ? folderProblem(cloneRoot) : '';
  if (root) problems['github.cloneRoot'] = root;
  return problems;
}

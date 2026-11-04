import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { COMMAND_NAMES, type Command, type Commands } from './ipc';
import type {
  AgentId,
  AppSettings,
  LaunchRequest,
  RecentSession,
} from './types';

const root = fileURLToPath(new URL('../..', import.meta.url));
const read = (path: string) => readFileSync(join(root, path), 'utf8');

/**
 * Names in `generate_handler![…]`, each by its last path segment. Comments
 * and `#[…]` attributes (e.g. a `#[cfg(…)]` entry) go first, so neither a `]`
 * nor a comma inside them ends or splits the list; the list runs up to the
 * macro's closing `])`.
 */
function registeredCommands(source: string): string[] {
  const code = source
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .replace(/\/\/.*$/gm, '')
    .replace(/#\[[^\]]*\]/g, '');
  const body = /generate_handler!\[([\s\S]*?)\]\s*\)/.exec(code)?.[1];
  if (!body) throw new Error('generate_handler! not found in desktop.rs');
  return body
    .split(',')
    .map((entry) => entry.trim())
    .filter(Boolean)
    .map((path) => path.split('::').at(-1) ?? path);
}

/** Every Rust source of the shell, for the handler signatures. */
const RUST_SOURCES = readdirSync(join(root, 'src-tauri/src'), {
  recursive: true,
})
  .map(String)
  .filter((path) => path.endsWith('.rs'))
  .map((path) => read(join('src-tauri/src', path)));

/**
 * The camelCase argument names of a `#[tauri::command]` handler. Injected
 * parameters (app handle, state, window) are not arguments; generic
 * arguments such as `State<'_, StartupState>` hold commas.
 */
function handlerParams(command: string): string[] {
  // The attribute, then any doc comments or attributes, then the signature.
  const signature = new RegExp(
    `#\\[tauri::command[^\\]]*\\][^{;]*?pub (?:async )?fn ${command}\\(([^)]*)\\)`,
  );
  const sources = RUST_SOURCES.filter((source) => signature.test(source));
  expect(sources, `one Rust handler named ${command}`).toHaveLength(1);
  const params = signature.exec(sources[0]!)?.[1] ?? '';
  return params
    .replace(/<[^>]*>/g, '')
    .split(',')
    .map((param) => param.split(':')[0]!.trim())
    .filter((name) => name && !['app', 'state', 'window'].includes(name))
    .map((name) => name.replace(/_(\w)/g, (_, c: string) => c.toUpperCase()));
}

/**
 * One typed sample per command with arguments: a renamed field in `Commands`
 * is a type error here, a renamed Rust parameter fails the test.
 */
const ARGS: { [K in Command]?: Exclude<Commands[K]['args'], undefined> } = {
  set_autostart: { enabled: true },
  install_update: {
    onProgress: {} as Commands['install_update']['args']['onProgress'],
  },
  save_settings: { settings: {} as AppSettings },
  launch_session: { request: {} as LaunchRequest },
  remove_recent: { id: 'local:claude:c:\\x' },
  restore_recent: { recent: {} as RecentSession },
  pick_folder: { initial: null },
  install_agent: { agent: 'claude' as AgentId },
  open_folder: { path: 'C:\\x' },
  github_connect_token: { token: 'ghp_x' },
  github_import_local: { login: 'demo-user' },
};

/** A Tauri invoke call, with or without a type argument. */
const RAW_CALL = /\binvoke\s*(?:<.*?>)?\s*\(/;
/** Any import that names invoke from the Tauri core module. */
const RAW_IMPORT =
  /import\s*(?:type\s*)?\{[^}]*\binvoke\b[^}]*\}\s*from\s*'@tauri-apps\/api\/core'/;

describe('IPC contract', () => {
  it('reads handler names past comments and attributes', () => {
    const source = `
      .invoke_handler(tauri::generate_handler![
          snapshot, // trailing note, with a comma
          /* removed_command, ] */
          #[cfg(feature = "desktop")]
          crate::desktop_startup::set_autostart,
          #[cfg(any(windows, target_os = "macos"))] hide_panel
      ])
      .run(tauri::generate_context!())`;
    expect(registeredCommands(source)).toEqual([
      'snapshot',
      'set_autostart',
      'hide_panel',
    ]);
  });

  it('types exactly the commands the native shell registers', () => {
    const registered = registeredCommands(read('src-tauri/src/desktop.rs'));
    expect(new Set(registered).size).toBe(registered.length);
    expect([...COMMAND_NAMES].sort()).toEqual([...registered].sort());
  });

  it('reaches the native shell only through ipc.ts', () => {
    const ipc = read('src/lib/ipc.ts');
    // The patterns must still find the one sanctioned call and import.
    expect(RAW_CALL.test(ipc) && RAW_IMPORT.test(ipc)).toBe(true);
    const offenders = readdirSync(join(root, 'src'), { recursive: true })
      .map(String)
      .filter((path) => /\.(?:ts|svelte)$/.test(path))
      .map((path) => join('src', path))
      .filter((path) => path !== join('src', 'lib', 'ipc.ts'))
      .filter((path) => {
        const source = read(path);
        return RAW_CALL.test(source) || RAW_IMPORT.test(source);
      });
    expect(offenders).toEqual([]);
  });

  it.each(Object.entries(ARGS))(
    'passes %s the arguments its Rust handler names',
    (command, args) => {
      expect(handlerParams(command)).toEqual(Object.keys(args));
    },
  );
});

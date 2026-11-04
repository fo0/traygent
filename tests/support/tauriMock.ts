import type { Page } from '@playwright/test';

/**
 * Labelled Tauri IPC mock for Playwright specs. It replaces the native shell
 * with an in-page `__TAURI_INTERNALS__` so the real panel code runs in its
 * native branch against synthetic data only.
 */
export type IpcArgs = Record<string, unknown> | undefined;
export type IpcCall = {
  cmd: string;
  args?: Record<string, unknown> | undefined;
};
/** Answers one IPC command; `undefined` resolves as `null`, a throw rejects. */
export type IpcHandler = (cmd: string, args: IpcArgs) => unknown;
/**
 * Runs once per document inside the page, so it must be self-contained: it may
 * only use its parameters and browser globals, never imports or outer scope.
 * Local variables persist across calls; `state` is the part a test may change.
 */
export type IpcFactory<A, S extends object> = (arg: A, state: S) => IpcHandler;

type MockRuntime = { calls: IpcCall[]; state: Record<string, unknown> };

declare global {
  interface Window {
    /** Present only while a spec installed the labelled IPC mock. */
    __traygentIpcMock?: MockRuntime;
  }
}

/** Browser-side shell shared by every mock; serialized into the init script. */
function shell(
  factory: (arg: unknown, state: Record<string, unknown>) => IpcHandler,
  arg: unknown,
  state: Record<string, unknown>,
) {
  const calls: IpcCall[] = [];
  const handle = factory(arg, state);
  let callback = 0;
  Object.assign(window, {
    isTauri: true,
    __traygentIpcMock: { calls, state },
    __TAURI_EVENT_PLUGIN_INTERNALS__: { unregisterListener() {} },
    __TAURI_INTERNALS__: {
      invoke: async (cmd: string, args?: Record<string, unknown>) => {
        calls.push({ cmd, args });
        if (cmd === 'plugin:event|listen') return ++callback;
        return (await handle(cmd, args)) ?? null;
      },
      transformCallback: () => ++callback,
      unregisterCallback() {},
      metadata: {
        currentWindow: { label: 'main' },
        currentWebview: { label: 'main', windowLabel: 'main' },
      },
    },
  });
}

/** Typed access to the mutable state of one installed mock. */
export interface TauriMock<S extends object> {
  /** Changes the state the factory reads on its next call. */
  configure(values: Partial<S>): Promise<void>;
  /** Reads one state value, e.g. a settled counter for `expect.poll`. */
  read<K extends keyof S & string>(key: K): Promise<S[K]>;
}

/** Installs the mock for every document the page loads, including reloads. */
export async function installTauriMock<A, S extends object>(
  page: Page,
  factory: IpcFactory<A, S>,
  options: { arg: A; state: S },
): Promise<TauriMock<S>> {
  await page.addInitScript({
    content: `(${shell.toString()})(${factory.toString()}, ${JSON.stringify(
      options.arg,
    )}, ${JSON.stringify(options.state)});`,
  });
  return {
    async configure(values) {
      await page.evaluate((values) => {
        const mock = window.__traygentIpcMock;
        if (!mock) throw new Error('The labelled IPC mock is not installed.');
        Object.assign(mock.state, values);
      }, values);
    },
    async read<K extends keyof S & string>(key: K) {
      const value = await page.evaluate(
        (key) => window.__traygentIpcMock?.state[key],
        key,
      );
      // The page boundary erases types; the state was installed as an `S`.
      return value as S[K];
    },
  };
}

/** Recorded calls, optionally only one command; filtered in the page. */
export function readCalls(page: Page, cmd?: string): Promise<IpcCall[]> {
  return page.evaluate(
    (cmd) =>
      (window.__traygentIpcMock?.calls ?? [])
        .filter((call) => cmd === undefined || call.cmd === cmd)
        .map(({ cmd, args }) => ({ cmd, args })),
    cmd,
  );
}

/** Command names in call order; never transfers argument objects. */
export function readCommands(page: Page): Promise<string[]> {
  return page.evaluate(() =>
    (window.__traygentIpcMock?.calls ?? []).map((call) => call.cmd),
  );
}

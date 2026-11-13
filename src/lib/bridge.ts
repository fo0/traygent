import { isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { Bridge, ObserveSnapshots } from './bridgeTypes';
import { createDemoBridge } from './demo';
import { call } from './ipc';
import type { Snapshot } from './types';

export const native = isTauri();

/** Native commands; the browser preview serves the labelled demo instead. */
const nativeBridge: Bridge = {
  snapshot: () => call('snapshot'),
  refreshDetection: () => call('refresh_detection'),
  saveSettings: (settings) => call('save_settings', { settings }),
  launchSession: (request) => call('launch_session', { request }),
  removeRecent: (id) => call('remove_recent', { id }),
  restoreRecent: (recent) => call('restore_recent', { recent }),
  updateRecent: (id, options, ports, profile) =>
    call('update_recent', { id, options, ports, profile: profile ?? null }),
  pickFolder: (initial) => call('pick_folder', { initial }),
  installAgent: (agent) => call('install_agent', { agent }),
  openFolder: (path) => call('open_folder', { path }),
  githubConnectToken: (token) => call('github_connect_token', { token }),
  githubLocalAccounts: () => call('github_local_accounts'),
  githubImportLocal: (login) => call('github_import_local', { login }),
  githubDisconnect: () => call('github_disconnect'),
  githubRefreshRepos: () => call('github_refresh_repos'),
  updateAgents: () => call('update_agents'),
  syncProfiles: () => call('sync_profiles'),
  profileLogin: (agent, profile) => call('profile_login', { agent, profile }),
};

/** Snapshots the native shell pushes (`traygent:snapshot`). */
const observeNative: ObserveSnapshots = async (callback) => {
  try {
    return await listen<Snapshot>('traygent:snapshot', (event) =>
      callback(event.payload),
    );
  } catch {
    /* A shell without event permissions can still read native snapshots. */
    return () => {};
  }
};

const demo = native ? null : createDemoBridge();

export const bridge: Bridge = demo ?? nativeBridge;
/** Natively the shell's events; in the browser the demo's simulated detection. */
export const observeSnapshots: ObserveSnapshots = demo
  ? demo.observe
  : observeNative;

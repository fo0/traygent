import { describe, expect, it, vi } from 'vitest';
import type { Bridge } from './bridgeTypes';
import { demoSnapshot } from './demoData';
import {
  LAUNCH_NOTICE,
  Panel,
  UNDO_TIMEOUT,
  installNote,
} from './panel.svelte';
import type { LaunchOutcome, LaunchRequest, Snapshot } from './types';

/** A synthetic native snapshot: the demo data, labelled not-demo. */
function snapshot(value: Partial<Snapshot> = {}): Snapshot {
  const base = demoSnapshot(Date.parse('2026-11-03T09:00:00Z'));
  delete base.checkedAt;
  return { ...base, demo: false, ...value };
}
const idle = async () => () => {};

function bridgeWith(overrides: Partial<Bridge> = {}): Bridge {
  return {
    snapshot: async () => snapshot(),
    refreshDetection: async () => snapshot(),
    saveSettings: async (settings) => snapshot({ settings }),
    launchSession: async () => ({
      recent: snapshot().recents[0]!,
      snapshot: snapshot(),
    }),
    removeRecent: async (id) =>
      snapshot({ recents: snapshot().recents.filter((r) => r.id !== id) }),
    restoreRecent: async () => snapshot(),
    updateRecent: async () => snapshot(),
    pickFolder: async () => null,
    installAgent: async () => {},
    openFolder: async () => {},
    githubConnectToken: async () => snapshot(),
    githubLocalAccounts: async () => ({ accounts: [] }),
    githubImportLocal: async () => snapshot(),
    githubDisconnect: async () => snapshot(),
    githubRefreshRepos: async () => snapshot(),
    updateAgents: async () => snapshot(),
    syncProfiles: async () => snapshot(),
    profileLogin: async () => {},
    ...overrides,
  };
}

function panelWith(overrides: Partial<Bridge> = {}, native = true) {
  return new Panel({ bridge: bridgeWith(overrides), native, observe: idle });
}

/** A panel whose pushed snapshots the test sends itself. */
async function pushingPanel(overrides: Partial<Bridge> = {}) {
  let push: (value: Snapshot) => void = () => {};
  const panel = new Panel({
    bridge: bridgeWith(overrides),
    native: true,
    observe: async (callback) => {
      push = callback;
      return () => {};
    },
  });
  panel.load();
  await vi.waitFor(() => expect(panel.snapshot).not.toBeNull());
  return { panel, push: (value: Snapshot) => push(value) };
}
const texts = (panel: Panel) => panel.notices.items.map((item) => item.text);

const request: LaunchRequest = {
  agent: 'claude',
  env: 'local',
  folder: 'C:\\Users\\demo\\Projekte\\traygent',
  options: {
    model: 'default',
    effort: 'default',
    permission: 'auto',
    extraArgs: '',
  },
  session: 'new',
  ports: [],
};

describe('Panel', () => {
  it('is labelled demo only by its snapshot, natively never before it', async () => {
    const native = panelWith();
    expect(native.demo).toBe(false);
    expect(panelWith({}, false).demo).toBe(true);
    const loaded = vi.fn();
    native.load(loaded);
    await vi.waitFor(() => expect(loaded).toHaveBeenCalled());
    expect(native.demo).toBe(false);
  });

  it('shows a load error and keeps it until a refresh succeeds', async () => {
    const panel = panelWith({
      snapshot: () => Promise.reject('Dienst nicht verfügbar.'),
      refreshDetection: async () =>
        snapshot({ checkedAt: '2026-11-03T09:00:00Z' }),
    });
    panel.load();
    await vi.waitFor(() => expect(panel.error).toBe('Dienst nicht verfügbar.'));
    const next = await panel.refresh();
    expect(next?.checkedAt).toBe('2026-11-03T09:00:00Z');
    expect(panel.snapshot?.checkedAt).toBe('2026-11-03T09:00:00Z');
    expect(panel.error).toBe('');
    // An answer without `detecting` started no detection: nothing to announce.
    expect(texts(panel)).toEqual([]);
  });

  it('runs one refresh at a time and drops answers after the cleanup', async () => {
    let resolve: (value: Snapshot) => void = () => {};
    const panel = panelWith({
      refreshDetection: () => new Promise((done) => (resolve = done)),
    });
    const stop = panel.load();
    const first = panel.refresh();
    expect(await panel.refresh()).toBeNull();
    stop();
    resolve(snapshot({ checkedAt: 'late' }));
    expect(await first).toBeNull();
    expect(panel.snapshot?.checkedAt).toBeUndefined();
  });

  it('shows a snapshot unless it is older than the shown one', () => {
    const panel = panelWith();
    expect(panel.accept(snapshot({ revision: 5, checkedAt: 'five' }))).toBe(
      true,
    );
    expect(panel.accept(snapshot({ revision: 4, checkedAt: 'four' }))).toBe(
      false,
    );
    expect(panel.snapshot?.checkedAt).toBe('five');
    // An equal revision is shown: mocks and replays answer with the same one.
    expect(panel.accept(snapshot({ revision: 5, checkedAt: 'again' }))).toBe(
      true,
    );
    expect(panel.snapshot?.checkedAt).toBe('again');
  });

  it('announces a detection only when a finished, newer snapshot follows its answer', async () => {
    const { panel, push } = await pushingPanel({
      refreshDetection: async () => snapshot({ revision: 2, detecting: true }),
    });
    await panel.refresh();
    expect(panel.detecting).toBe(true);
    // A late push of an older detection, and one at the answer's revision.
    push(snapshot({ revision: 1, detecting: false, checkedAt: 'stale' }));
    push(snapshot({ revision: 2, detecting: false }));
    expect(texts(panel)).toEqual([]);
    expect(panel.snapshot?.checkedAt).not.toBe('stale');
    push(snapshot({ revision: 3, detecting: false, checkedAt: 'finished' }));
    expect(panel.detecting).toBe(false);
    expect(texts(panel)).toEqual(['Aktualisiert.']);
    // Later snapshots announce nothing more.
    push(snapshot({ revision: 4, detecting: false }));
    expect(texts(panel)).toEqual(['Aktualisiert.']);
  });

  it('push before response: keeps the finished push and announces it', async () => {
    let answer: (value: Snapshot) => void = () => {};
    const { panel, push } = await pushingPanel({
      refreshDetection: () => new Promise((done) => (answer = done)),
    });
    const running = panel.refresh();
    push(snapshot({ revision: 3, detecting: false, checkedAt: 'finished' }));
    expect(texts(panel)).toEqual([]);
    answer(snapshot({ revision: 2, detecting: true }));
    await running;
    expect(panel.snapshot?.checkedAt).toBe('finished');
    expect(panel.detecting).toBe(false);
    expect(texts(panel)).toEqual(['Aktualisiert.']);
  });

  it('response after push: an older command answer never replaces a push', async () => {
    let answer: (value: LaunchOutcome) => void = () => {};
    const { panel, push } = await pushingPanel({
      launchSession: () => new Promise((done) => (answer = done)),
    });
    const launching = panel.launch(request);
    push(snapshot({ revision: 7, checkedAt: 'pushed' }));
    answer({
      recent: snapshot().recents[0]!,
      snapshot: snapshot({ revision: 6, checkedAt: 'answer' }),
    });
    expect(await launching).toBe(true);
    expect(panel.snapshot?.checkedAt).toBe('pushed');
  });

  it('drops a load answer older than a snapshot pushed before it', async () => {
    let answer: (value: Snapshot) => void = () => {};
    let push: (value: Snapshot) => void = () => {};
    const panel = new Panel({
      bridge: bridgeWith({
        snapshot: () => new Promise((done) => (answer = done)),
      }),
      native: true,
      observe: async (callback) => {
        push = callback;
        return () => {};
      },
    });
    const loaded = vi.fn();
    panel.load(loaded);
    push(snapshot({ revision: 9, checkedAt: 'pushed' }));
    answer(snapshot({ revision: 8, checkedAt: 'load' }));
    await vi.waitFor(() => expect(loaded).toHaveBeenCalled());
    expect(panel.snapshot?.checkedAt).toBe('pushed');
  });

  it('launches once at a time and shows a short success notice', async () => {
    const launchSession = vi.fn(bridgeWith().launchSession);
    const panel = panelWith({ launchSession });
    const first = panel.launch(request);
    expect(await panel.launch(request)).toBe(false);
    expect(await first).toBe(true);
    expect(launchSession).toHaveBeenCalledTimes(1);
    const notice = panel.notices.get(LAUNCH_NOTICE);
    expect(notice).toMatchObject({
      text: 'Claude Code gestartet.',
      tone: 'subtle',
      timeout: 3000,
    });
  });

  it('shows a start with a warning as a warning that stays: it did start', async () => {
    const warning = 'Profil „Team“ ist nicht abgeglichen.';
    const panel = panelWith({
      launchSession: async () => ({
        ...(await bridgeWith().launchSession(request)),
        warning,
      }),
    });
    expect(await panel.launch(request)).toBe(true);
    expect(panel.notices.get(LAUNCH_NOTICE)).toMatchObject({
      text: `Claude Code gestartet. ${warning}`,
      tone: 'warning',
      timeout: undefined,
    });
  });

  it('syncs profiles once at a time and opens a sign-in', async () => {
    let resolve: (value: Snapshot) => void = () => {};
    const syncProfiles = vi.fn(
      () => new Promise<Snapshot>((done) => (resolve = done)),
    );
    const profileLogin = vi.fn(async () => {});
    const panel = panelWith({ syncProfiles, profileLogin });
    const first = panel.profiles.sync();
    expect(panel.profiles.syncing).toBe(true);
    expect(await panel.profiles.sync()).toBe(false);
    const synced = snapshot({ revision: 7 });
    resolve(synced);
    expect(await first).toBe(true);
    expect(panel.snapshot).toBe(synced);
    expect(panel.profiles.syncing).toBe(false);
    expect(syncProfiles).toHaveBeenCalledTimes(1);
    await panel.profiles.login('claude', 'team');
    expect(profileLogin).toHaveBeenCalledWith('claude', 'team');
    syncProfiles.mockRejectedValueOnce('Profil ist gesperrt.');
    await expect(panel.profiles.sync()).rejects.toBe('Profil ist gesperrt.');
    expect(panel.profiles.syncing).toBe(false);
  });

  it('keeps a failed start as an error notice with the German message', async () => {
    const panel = panelWith({
      launchSession: () => Promise.reject('Ordner existiert nicht.'),
    });
    expect(await panel.launch(request)).toBe(false);
    expect(panel.notices.get(LAUNCH_NOTICE)).toMatchObject({
      text: 'Ordner existiert nicht.',
      tone: 'danger',
      timeout: undefined,
    });
  });

  it('offers „Rückgängig“ for 8 s after a removal and restores the entry', async () => {
    const restoreRecent = vi.fn(async () => snapshot());
    const panel = panelWith({ restoreRecent });
    panel.accept(snapshot());
    const removed = panel.snapshot!.recents[1]!;
    await panel.removeRecent(removed);
    expect(panel.snapshot?.recents.some((r) => r.id === removed.id)).toBe(
      false,
    );
    const notice = panel.notices.get(`recent:${removed.id}`);
    expect(notice?.timeout).toBe(UNDO_TIMEOUT);
    expect(notice?.action?.label).toBe('Rückgängig');
    notice?.action?.run();
    await vi.waitFor(() => expect(restoreRecent).toHaveBeenCalledWith(removed));
    expect(panel.notices.get(`recent:${removed.id}`)).toBeUndefined();
  });

  it('copies a folder path and says so, or says the copy failed', async () => {
    const writeText = vi.fn<(text: string) => Promise<void>>(async () => {});
    vi.stubGlobal('navigator', { clipboard: { writeText } });
    try {
      const panel = panelWith();
      await panel.copyFolder('C:\\Users\\demo\\Projekte\\traygent');
      expect(writeText).toHaveBeenCalledWith(
        'C:\\Users\\demo\\Projekte\\traygent',
      );
      expect(panel.notices.get('panel')).toMatchObject({
        text: 'Pfad kopiert.',
        tone: 'subtle',
        timeout: 3000,
      });
      writeText.mockRejectedValueOnce(new Error('NotAllowedError'));
      await panel.copyFolder('C:\\x');
      expect(panel.notices.get('panel')).toMatchObject({
        text: 'Pfad konnte nicht kopiert werden.',
        tone: 'danger',
        timeout: undefined,
      });
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it('stores an edited recent and rejects with the German message', async () => {
    const edited = snapshot({ revision: 7 });
    const updateRecent = vi.fn(async () => edited);
    const panel = panelWith({ updateRecent });
    panel.accept(snapshot());
    const recent = panel.snapshot!.recents[0]!;
    const options = { ...recent.options, permission: 'bypass' as const };
    await panel.updateRecent(recent, options, [], 'team');
    expect(updateRecent).toHaveBeenCalledWith(recent.id, options, [], 'team');
    expect(panel.snapshot).toBe(edited);
    updateRecent.mockRejectedValueOnce(new Error('Ungültiges Modell.'));
    await expect(panel.updateRecent(recent, options, [])).rejects.toThrow(
      'Ungültiges Modell.',
    );
  });

  it('starts an agent update run and rejects with the German message', async () => {
    const started = snapshot({ revision: 9 });
    const updateAgents = vi.fn(async () => started);
    const panel = panelWith({ updateAgents });
    await panel.updateAgents();
    expect(panel.snapshot).toBe(started);
    updateAgents.mockRejectedValueOnce('Agenten werden bereits aktualisiert.');
    await expect(panel.updateAgents()).rejects.toBe(
      'Agenten werden bereits aktualisiert.',
    );
  });

  it('opens the install terminal without detecting again at once', async () => {
    const refreshDetection = vi.fn(async () => snapshot());
    const installAgent = vi.fn(async () => {});
    const panel = panelWith({ refreshDetection, installAgent });
    await panel.installAgent('opencode');
    expect(installAgent).toHaveBeenCalledWith('opencode');
    expect(refreshDetection).not.toHaveBeenCalled();
    panel.announceInstall();
    expect(panel.notices.get('panel')).toMatchObject({
      text: installNote(false),
      tone: 'subtle',
      timeout: 3000,
    });
    expect(installNote(false)).toBe(
      'Installation gestartet – danach aktualisieren (F5).',
    );
    await expect(
      panelWith({
        installAgent: () => Promise.reject('Terminal fehlt.'),
      }).installAgent('codex'),
    ).rejects.toBe('Terminal fehlt.');
  });
});

import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { StartupStatus } from './types';

const call = vi.fn();
vi.mock('./ipc', () => ({ call: (...args: unknown[]) => call(...args) }));
const { Autostart, startupNote } = await import('./autostart.svelte');

const status = (
  value: StartupStatus['status'],
  registered = value !== 'disabled',
): StartupStatus => ({ status: value, registered });

/** A native dialog whose first read saw `initial`. */
async function opened(initial: StartupStatus['status'] = 'disabled') {
  call.mockResolvedValueOnce(status(initial));
  const autostart = new Autostart(true);
  await autostart.read();
  return autostart;
}

beforeEach(() => call.mockReset());

describe('startupNote', () => {
  it('stays silent where the switch shows the state itself', () => {
    expect(startupNote(status('enabled'), false)).toBe('');
    expect(startupNote(status('disabled'), false)).toBe('');
  });
  it('names a status the switch cannot show in one line', () => {
    expect(startupNote(null, true)).toBe('Status wird gelesen …');
    expect(startupNote(null, false)).toBe('Status unbekannt.');
    expect(startupNote(status('conflict'), false)).toMatch(
      /Es wird nichts überschrieben\.$/,
    );
    expect(startupNote(status('disabledByWindows'), false)).toMatch(
      /^In Windows deaktiviert/,
    );
  });
});

describe('Autostart', () => {
  it('stages the switch without writing and drops a choice that matches Windows', async () => {
    const autostart = await opened();
    autostart.stage(true);
    expect(autostart.pending).toBe(true);
    expect(autostart.checked).toBe(true);
    autostart.stage(false);
    expect(autostart.pending).toBeNull();
    expect(call).toHaveBeenCalledTimes(1);
  });

  it('never stages a status Windows or another entry owns', async () => {
    const autostart = await opened('disabledByWindows');
    autostart.stage(true);
    expect(autostart.pending).toBeNull();
    expect(autostart.editable).toBe(false);
  });

  it('writes a staged choice once on apply and accepts the matching readback', async () => {
    const autostart = await opened();
    autostart.stage(true);
    call.mockResolvedValueOnce(status('enabled'));
    await autostart.apply();
    expect(call).toHaveBeenLastCalledWith('set_autostart', { enabled: true });
    expect(autostart.pending).toBeNull();
    expect(autostart.current?.status).toBe('enabled');
    await autostart.apply();
    expect(call).toHaveBeenCalledTimes(2);
  });

  it('rejects with the readback line when Windows does not follow', async () => {
    const autostart = await opened();
    autostart.stage(true);
    call.mockResolvedValueOnce(status('disabledByWindows'));
    await expect(autostart.apply()).rejects.toThrow(/^In Windows deaktiviert/);
  });

  it('keeps no optimistic state after a failed write', async () => {
    const autostart = await opened();
    autostart.stage(true);
    call.mockRejectedValueOnce(
      'Windows-Autostart konnte nicht geändert werden.',
    );
    await expect(autostart.apply()).rejects.toBe(
      'Windows-Autostart konnte nicht geändert werden.',
    );
    expect(autostart.current).toBeNull();
    expect(autostart.checked).toBe(false);
  });

  it('keeps a staged choice across a re-read only while it still differs', async () => {
    const autostart = await opened();
    autostart.stage(true);
    call.mockResolvedValueOnce(status('disabled'));
    await autostart.read();
    expect(autostart.pending).toBe(true);
    call.mockResolvedValueOnce(status('enabled'));
    await autostart.read();
    expect(autostart.pending).toBeNull();
  });

  it('touches nothing outside the desktop app', async () => {
    const autostart = new Autostart(false);
    await autostart.read();
    autostart.stage(true);
    await autostart.apply();
    expect(call).not.toHaveBeenCalled();
  });
});

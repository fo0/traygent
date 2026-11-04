import { describe, expect, it, vi } from 'vitest';
import {
  RESTART_COOLDOWN,
  RestartGate,
  repeatClick,
  repeatKey,
} from './rowStart';

/** A start the test settles by hand. */
function pending() {
  let settle: (started: boolean) => void = () => {};
  const start = vi.fn(
    () => new Promise<boolean>((done) => (settle = (value) => done(value))),
  );
  return { start, settle: (value: boolean) => settle(value) };
}
const flush = () => new Promise((done) => setTimeout(done));

describe('one start per press', () => {
  it('a row click starts once; the rest of a double-click never does', () => {
    expect(repeatClick({ detail: 1 })).toBe(false);
    expect(repeatClick({ detail: 2 })).toBe(true);
    expect(repeatClick({ detail: 3 })).toBe(true);
    // Enter and Space on the button click with detail 0.
    expect(repeatClick({ detail: 0 })).toBe(false);
  });

  it('a held Enter or Space never starts again', () => {
    expect(repeatKey({ key: 'Enter', repeat: false })).toBe(false);
    expect(repeatKey({ key: 'Enter', repeat: true })).toBe(true);
    expect(repeatKey({ key: ' ', repeat: true })).toBe(true);
    expect(repeatKey({ key: 'Tab', repeat: true })).toBe(false);
  });

  it('refuses a second start of the same row while it runs and cools down', async () => {
    let now = 1000;
    const gate = new RestartGate(() => now);
    const { start, settle } = pending();
    expect(gate.run('a', start)).toBe(true);
    expect(gate.run('a', start)).toBe(false);
    settle(true);
    await flush();
    now += RESTART_COOLDOWN - 1;
    expect(gate.run('a', start)).toBe(false);
    now += 1;
    expect(gate.run('a', start)).toBe(true);
    expect(start).toHaveBeenCalledTimes(2);
  });

  it('another row starts right away', () => {
    const gate = new RestartGate(() => 0);
    expect(gate.run('a', pending().start)).toBe(true);
    expect(gate.run('b', pending().start)).toBe(true);
  });

  it('a start that did not happen leaves no cooldown', async () => {
    const gate = new RestartGate(() => 0);
    const refused = pending();
    gate.run('a', refused.start);
    refused.settle(false);
    await flush();
    expect(gate.run('a', pending().start)).toBe(true);
    const failing = vi.fn(() => Promise.reject(new Error('weg')));
    const gateB = new RestartGate(() => 0);
    gateB.run('b', failing);
    await flush();
    expect(gateB.run('b', pending().start)).toBe(true);
  });
});

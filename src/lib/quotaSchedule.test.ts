import { describe, expect, it } from 'vitest';
import {
  Backoff,
  ProviderQueue,
  READ_GAP,
  READ_JITTER,
  backoffDelay,
  rateLimited,
} from './quotaSchedule';

const MINUTE = 60_000;

describe('quota schedule (#100)', () => {
  it('doubles the backoff per 429 in a row up to an hour', () => {
    expect([1, 2, 3, 4, 5, 6, 12].map((n) => backoffDelay(n) / MINUTE)).toEqual(
      [5, 10, 20, 40, 60, 60, 60],
    );
    // A provider asking for longer wins; a day at most; nonsense is ignored.
    expect(backoffDelay(1, 2 * 3_600) / MINUTE).toBe(120);
    expect(backoffDelay(5, 600) / MINUTE).toBe(60);
    expect(backoffDelay(1, 10 ** 9) / MINUTE).toBe(24 * 60);
    expect(backoffDelay(1, -5) / MINUTE).toBe(5);
    expect(backoffDelay(1, Number.NaN) / MINUTE).toBe(5);
  });

  it('counts the streak per account and ends it on a good read', () => {
    const backoff = new Backoff();
    expect(backoff.until('a')).toBe(0);
    expect(backoff.limited('a', 0)).toBe(5 * MINUTE);
    expect(backoff.limited('a', 0, 0)).toBe(10 * MINUTE);
    expect(backoff.limited('b', 0)).toBe(5 * MINUTE);
    backoff.clear('a');
    expect(backoff.until('a')).toBe(0);
    expect(backoff.limited('a', 0)).toBe(5 * MINUTE);
  });

  it('frees a provider a gap plus jitter after its read ends', () => {
    let draw = 0.5;
    const queue = new ProviderQueue(() => draw);
    expect(queue.freeAt('claude')).toBe(0);
    queue.start('claude');
    expect(queue.freeAt('claude')).toBe(Infinity);
    expect(queue.freeAt('codex')).toBe(0);
    queue.finish('claude', 1_000);
    expect(queue.freeAt('claude')).toBe(1_000 + READ_GAP + READ_JITTER / 2);
    draw = 7;
    queue.start('claude');
    queue.finish('claude', 0);
    expect(queue.freeAt('claude')).toBe(READ_GAP + READ_JITTER);
  });

  it('recognizes a limited read only by its Retry-After field', () => {
    expect(rateLimited({ windows: {}, detail: 'x', retryAfterSecs: 0 })).toBe(
      true,
    );
    expect(rateLimited({ windows: {}, detail: 'x' })).toBe(false);
    expect(rateLimited(undefined)).toBe(false);
  });
});

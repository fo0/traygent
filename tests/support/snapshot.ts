import { demoSnapshot } from '../../src/lib/demoData';
import type { Snapshot } from '../../src/lib/types';

/**
 * A synthetic snapshot for the labelled native-IPC mock: the browser demo's
 * invented data, sent with `demo: false` as the native shell would.
 */
export function nativeSnapshot(overrides: Partial<Snapshot> = {}): Snapshot {
  return { ...demoSnapshot(), demo: false, ...overrides };
}

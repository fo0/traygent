import { describe, expect, it } from 'vitest';
import { agentUpdateText } from './agentUpdates';
import type { AgentUpdateStatus } from './types';

const at = '2026-11-10T08:00:00Z';
const status = (value: Partial<AgentUpdateStatus>): AgentUpdateStatus => ({
  agent: 'claude',
  state: 'current',
  at,
  ...value,
});

describe('agentUpdateText', () => {
  it('names every state in German', () => {
    expect(agentUpdateText(status({ state: 'running' }))).toEqual({
      text: 'wird aktualisiert …',
    });
    expect(agentUpdateText(status({ state: 'inUse' }))).toEqual({
      text: 'in Benutzung – übersprungen',
    });
    expect(agentUpdateText(status({ state: 'failed' }))).toEqual({
      text: 'fehlgeschlagen',
      tone: 'fail',
    });
  });

  it('shows the new version of an update, the old one in the tooltip and to assistive tech', () => {
    expect(
      agentUpdateText(
        status({ state: 'updated', from: '2.1.317', version: '2.1.318' }),
      ),
    ).toEqual({
      text: 'aktualisiert auf 2.1.318',
      title: '2.1.317 → 2.1.318',
      detail: 'vorher 2.1.317',
    });
    expect(
      agentUpdateText(status({ state: 'updated', version: '2.1.318' })),
    ).toEqual({ text: 'aktualisiert auf 2.1.318' });
    expect(agentUpdateText(status({ state: 'updated' }))).toEqual({
      text: 'aktualisiert',
    });
  });

  it('adds a known version to „aktuell“', () => {
    expect(agentUpdateText(status({ version: '0.166.0' }))).toEqual({
      text: 'aktuell · 0.166.0',
    });
    expect(agentUpdateText(status({}))).toEqual({ text: 'aktuell' });
  });
});

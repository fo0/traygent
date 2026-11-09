/*
 * Synthetic „Zuletzt“ sessions of the browser preview. Folders, repositories
 * and times are invented and never read from the machine.
 */
import type { AppSettings, RecentSession } from './types';
import { recentId } from './recents';
import { DEMO_CLONE_ROOT, DEMO_LOGIN } from './demoGithub';

export const DEMO_HOME = 'C:\\Users\\demo';

const MINUTE = 60_000;

/** Recent sessions relative to `now`, newest first, options over `defaults`. */
export function demoRecents(
  now: number,
  defaults: AppSettings['agents'],
): RecentSession[] {
  const entries: [
    folder: string,
    agent: RecentSession['agent'],
    env: RecentSession['env'],
    options: Partial<RecentSession['options']>,
    ago: number,
    starts: number,
    ports?: string[],
    repo?: string,
  ][] = [
    [`${DEMO_HOME}\\Projekte\\traygent`, 'claude', 'local', {}, 5, 14],
    [
      `${DEMO_CLONE_ROOT}\\${DEMO_LOGIN}\\repo-watch`,
      'codex',
      'local',
      { model: 'gpt-6.1-sol', effort: 'high' },
      47,
      6,
      [],
      'repo-watch',
    ],
    [
      'D:\\Arbeit\\kunden-portal',
      'claude',
      'docker',
      { model: 'opus', permission: 'bypass' },
      3 * 60,
      9,
      ['127.0.0.1:3000:3000', '127.0.0.1:5173:5173'],
    ],
    [
      `${DEMO_HOME}\\Projekte\\agent-tools`,
      'opencode',
      'docker',
      { model: 'opencode/big-pickle' },
      26 * 60,
      3,
    ],
    [
      'D:\\Arbeit\\infra\\terraform-module',
      'codex',
      'docker',
      { permission: 'auto' },
      3 * 24 * 60,
      2,
    ],
    [
      `${DEMO_HOME}\\Notizen`,
      'claude',
      'local',
      { effort: 'low', permission: 'ask' },
      9 * 24 * 60,
      1,
    ],
  ];
  return entries.map(
    ([folder, agent, env, options, ago, starts, ports, repo]) => ({
      id: recentId(folder, agent, env),
      agent,
      env,
      folder,
      ...(repo && { repo: { owner: DEMO_LOGIN, name: repo } }),
      options: { ...defaults[agent], ...options },
      ports: ports ?? [],
      lastStartedAt: new Date(now - ago * MINUTE).toISOString(),
      starts,
    }),
  );
}

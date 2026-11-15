/*
 * What a profile row in Settings → Agenten → „Profile“ shows after its name
 * (#41). Rows are told apart by what differs (UX rule 3): when another
 * signed-in profile of the agent has the same account but another
 * organization, the row shows its organization instead of the account; the
 * tooltip always carries both, and the plan (#43) when the credentials name
 * one.
 */
import type { ProfileStatus } from './types';

export const SIGNED_OUT = 'Nicht angemeldet';
const SIGNED_IN = 'Angemeldet';

export interface RowAccount {
  /** The row's text. */
  text: string;
  /** The full values, for the tooltip. */
  title: string;
  /** Signed out: the quiet colour. */
  out: boolean;
}

/** A plan as shown: `max` → „Max“; `''` without one. */
export function planLabel(plan: string | undefined): string {
  return plan ? plan.charAt(0).toUpperCase() + plan.slice(1) : '';
}

const same = (a: string | undefined, b: string | undefined) =>
  !!a && !!b && a.toLowerCase() === b.toLowerCase();

/** The account part of `status`'s row among the agent's `statuses`. */
export function rowAccount(
  status: ProfileStatus | undefined,
  statuses: readonly ProfileStatus[],
): RowAccount {
  if (!status?.signedIn)
    return { text: SIGNED_OUT, title: SIGNED_OUT, out: true };
  const account = status.account ?? SIGNED_IN;
  const { organization } = status;
  const title = [account, organization, planLabel(status.plan)]
    .filter((part) => !!part)
    .join(' · ');
  const twin = statuses.some(
    (other) =>
      other.agent === status.agent &&
      other.id !== status.id &&
      other.signedIn &&
      same(other.account, status.account) &&
      other.organization !== organization,
  );
  return {
    text: twin && organization ? organization : account,
    title,
    out: false,
  };
}

/** Panel-side update flow; the IPC payloads live in types.ts. */
export type UpdatePhase =
  | 'idle'
  | 'checking'
  | 'current'
  | 'available'
  | 'downloading'
  | 'verifying'
  | 'installing';

/** German date of a release, or an empty string when unknown or malformed. */
export function releaseDate(value: string | null): string {
  const date = value ? new Date(value) : null;
  return date && Number.isFinite(date.getTime())
    ? date.toLocaleDateString('de-DE', {
        day: '2-digit',
        month: '2-digit',
        year: 'numeric',
      })
    : '';
}

const MAX_BULLETS = 5;
const MAX_BULLET_CHARS = 120;
const BULLET = /^\s*[*-]\s+(.+)$/;
/** GitHub's generated suffix: ` by @user in https://…/pull/12`. */
const AUTHOR_SUFFIX =
  /\s+(?:by\s+@[\w.-]+(?:\[bot\])?\s+)?in\s+https?:\/\/\S+\s*$/i;

/** Markdown inline syntax as plain text: links keep their label, code and emphasis marks go. */
function plain(text: string): string {
  return text
    .replace(/!?\[([^\]]*)\]\([^)]*\)/g, '$1')
    .replace(/`([^`]*)`/g, '$1')
    .replace(/(\*\*|\*|~~)(\S(?:.*?\S)?)\1/g, '$2')
    .replace(/\s+/g, ' ')
    .trim();
}

/**
 * Up to five short plain-text bullets from GitHub's generated release notes:
 * list items only, without author and pull-request suffix; headings,
 * „Full Changelog“ lines and the „New Contributors“ section are skipped.
 */
export function changelogSummary(notes: string): string[] {
  const bullets: string[] = [];
  let contributors = false;
  for (const line of notes.split(/\r?\n/)) {
    if (/^\s*#{1,6}\s/.test(line)) {
      contributors = /new contributors/i.test(line);
      continue;
    }
    const item = BULLET.exec(line)?.[1];
    if (!item || contributors || /full changelog/i.test(item)) continue;
    const text = plain(item.replace(AUTHOR_SUFFIX, ''));
    if (!text) continue;
    bullets.push(
      text.length > MAX_BULLET_CHARS
        ? `${text.slice(0, MAX_BULLET_CHARS - 1).trimEnd()}…`
        : text,
    );
    if (bullets.length === MAX_BULLETS) break;
  }
  return bullets;
}

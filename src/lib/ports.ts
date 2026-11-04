/*
 * Published ports of a Docker session (`docker run -p`), checked and
 * normalized in the panel so a typing error shows before the native side sees
 * it. They mirror src-tauri/src/ports.rs, which validates again, normalizes
 * and has the last word (docs/SPEC.md → Ports).
 *
 * Grammar: `PORT` | `HOST:CONTAINER` | `IP:HOST:CONTAINER`; each port a number
 * 1–65535 or a range `A-B` (`A <= B`, at most 100 ports, host and container of
 * equal length); an
 * optional `/tcp` or `/udp`; the IP is IPv4. The canonical form always names
 * the IP (`127.0.0.1` unless one was typed), drops the default `/tcp` and
 * writes a one-port range as the port.
 */

export const MAX_PORTS = 20;
/** The bind address of every entry typed without an IP: never the LAN. */
export const LOOPBACK = '127.0.0.1';
export const TOO_MANY = `Höchstens ${MAX_PORTS} Port-Angaben erlaubt.`;
export const DOCKER_ONLY =
  'Ports lassen sich nur für Docker-Sessions freigeben.';
/** A range covers at most this many ports. */
export const MAX_SPAN = 100;
/** Longer input is never a valid spec. */
const MAX_SPEC_CHARS = 64;
/** How much of a refused spec the error repeats. */
const SHOWN_CHARS = 32;

/** An inclusive port range; a single port has `from === to`. */
export type PortRange = { from: number; to: number };

export interface PortSpec {
  ip: string;
  host: PortRange;
  container: PortRange;
  udp: boolean;
}

const SAFE = /^[A-Za-z0-9.:\-/]*$/;
const DIGITS = /^[0-9]+$/;
/** Rust's `Ipv4Addr` parser: four decimal octets, no leading zeros. */
const OCTET = /^(?:0|[1-9][0-9]{0,2})$/;
/** Everything but printable ASCII: control, format (U+202E, U+200B) and the rest. */
const UNPRINTABLE = /[^\x20-\x7e]/gu;
/**
 * Rust's `str::trim` (Unicode `White_Space`), not JavaScript's: U+0085 is
 * trimmed, U+FEFF is not, so both sides see the same entry.
 */
const EDGE_SPACE =
  /^[\t\n\v\f\r \u0085\u00a0\u1680\u2000-\u200a\u2028\u2029\u202f\u205f\u3000]+|[\t\n\v\f\r \u0085\u00a0\u1680\u2000-\u200a\u2028\u2029\u202f\u205f\u3000]+$/g;
export const trimPort = (text: string) => text.replace(EDGE_SPACE, '');

/** The input for an error message: printable ASCII only, at most 32 characters. */
function shown(spec: string) {
  const clean = spec.replace(UNPRINTABLE, '');
  return clean.length > SHOWN_CHARS ? `${clean.slice(0, SHOWN_CHARS)}…` : clean;
}
export const invalidPort = (spec: string) =>
  `Port „${shown(trimPort(spec))}“ ist ungültig.`;

class PortError extends Error {}

function port(text: string, spec: string): number {
  if (!DIGITS.test(text)) throw new PortError(invalidPort(spec));
  // More than five digits is out of range whatever the value.
  const value = text.length <= 5 ? Number(text) : 0;
  if (value < 1 || value > 65535)
    throw new PortError(`Port „${shown(spec)}“ liegt außerhalb von 1–65535.`);
  return value;
}

function range(text: string, spec: string): PortRange {
  const dash = text.indexOf('-');
  const from = port(dash < 0 ? text : text.slice(0, dash), spec);
  const to = dash < 0 ? from : port(text.slice(dash + 1), spec);
  if (from > to) throw new PortError(invalidPort(spec));
  return { from, to };
}

const ipv4 = (text: string) => {
  const parts = text.split('.');
  return (
    parts.length === 4 &&
    parts.every((part) => OCTET.test(part) && Number(part) <= 255)
  );
};

function parse(spec: string): PortSpec {
  if ([...spec].length > MAX_SPEC_CHARS || !SAFE.test(spec))
    throw new PortError(invalidPort(spec));
  const slash = spec.indexOf('/');
  const address = slash < 0 ? spec : spec.slice(0, slash);
  const protocol = slash < 0 ? 'tcp' : spec.slice(slash + 1).toLowerCase();
  if (protocol !== 'tcp' && protocol !== 'udp')
    throw new PortError(invalidPort(spec));
  const parts = address.split(':');
  if (parts.length > 3) throw new PortError(invalidPort(spec));
  let ip = LOOPBACK;
  if (parts.length === 3) {
    ip = parts[0] ?? '';
    if (!ipv4(ip))
      throw new PortError(
        `IP-Adresse in „${shown(spec)}“ ist ungültig (nur IPv4).`,
      );
  }
  const host = range(parts.length === 1 ? address : (parts.at(-2) ?? ''), spec);
  const container = range(parts.at(-1) ?? '', spec);
  if (host.to - host.from !== container.to - container.from)
    throw new PortError(
      `Host- und Container-Bereich in „${shown(spec)}“ müssen gleich lang sein.`,
    );
  // Spans are equal by now, as natively; Docker starts one proxy per port.
  if (host.to - host.from + 1 > MAX_SPAN)
    throw new PortError(
      `Bereich in „${shown(spec)}“ umfasst mehr als ${MAX_SPAN} Ports.`,
    );
  return { ip, host, container, udp: protocol === 'udp' };
}

/** The parts of one entry, or the German reason it breaks the grammar. */
export function parsePort(
  text: string,
): { spec: PortSpec; problem: '' } | { spec?: undefined; problem: string } {
  try {
    return { spec: parse(trimPort(text)), problem: '' };
  } catch (error) {
    if (error instanceof PortError) return { problem: error.message };
    throw error;
  }
}

const rangeText = (value: PortRange) =>
  value.from === value.to ? `${value.from}` : `${value.from}-${value.to}`;

export function formatPort(spec: PortSpec): string {
  const suffix = spec.udp ? '/udp' : '';
  return `${spec.ip}:${rangeText(spec.host)}:${rangeText(spec.container)}${suffix}`;
}

/** `3000` → `127.0.0.1:3000:3000`; null when the entry is invalid. */
export function normalizePort(text: string): string | null {
  const { spec } = parsePort(text);
  return spec ? formatPort(spec) : null;
}

/** A list as natively stored: blanks dropped, each normalized, order kept. */
export function normalizePorts(list: readonly string[]): string[] {
  return list
    .map(trimPort)
    .filter(Boolean)
    .map((entry) => normalizePort(entry) ?? entry);
}

/** The normalized entry of a preset checkbox. */
export const presetPort = (value: number) => `${LOOPBACK}:${value}:${value}`;

/** Docker binds a host port once per protocol whatever the IP. */
function sharedHostPort(a: PortSpec, b: PortSpec): number | null {
  const from = Math.max(a.host.from, b.host.from);
  return a.udp === b.udp && from <= Math.min(a.host.to, b.host.to)
    ? from
    : null;
}

/**
 * Why a list would be refused, checked in the native order: more than 20
 * entries, then the first invalid entry or host port bound twice on one
 * protocol (overlapping ranges included). Blank entries are dropped, as
 * natively. Empty when the native side may take it.
 */
export function portListProblem(list: readonly string[]): string {
  const entries = list.map(trimPort).filter(Boolean);
  if (entries.length > MAX_PORTS) return TOO_MANY;
  const specs: PortSpec[] = [];
  for (const entry of entries) {
    const { spec, problem } = parsePort(entry);
    if (!spec) return problem;
    for (const other of specs) {
      const shared = sharedHostPort(other, spec);
      if (shared !== null)
        return `Host-Port ${shared}${spec.udp ? '/udp' : ''} ist doppelt belegt.`;
    }
    specs.push(spec);
  }
  return '';
}

/** Accessible name and tooltip part of an entry bound to another IP than 127.0.0.1. */
export const EXPOSED = 'im Netzwerk erreichbar';

/** The bind address of an entry that does not bind to 127.0.0.1, else null. */
export function exposedIp(entry: string): string | null {
  const { spec } = parsePort(entry);
  return spec && spec.ip !== LOOPBACK ? spec.ip : null;
}

/**
 * Short form for a chip: the host side, e.g. `3000` or `8080-8081/udp`, led
 * by the IP when it is not 127.0.0.1 (`0.0.0.0:3000`).
 */
export function portLabel(entry: string): string {
  const { spec } = parsePort(entry);
  if (!spec) return entry;
  const ip = spec.ip === LOOPBACK ? '' : `${spec.ip}:`;
  return `${ip}${rangeText(spec.host)}${spec.udp ? '/udp' : ''}`;
}

/**
 * Adds a typed entry to `list`: its normalized form at the end, or the
 * German reason it cannot be added. An entry already listed is no change.
 */
export function addPort(
  list: readonly string[],
  text: string,
): { list: string[]; problem: '' } | { list?: undefined; problem: string } {
  const { spec, problem } = parsePort(text);
  if (!spec) return { problem };
  const normalized = formatPort(spec);
  if (list.includes(normalized)) return { list: [...list], problem: '' };
  const next = [...list, normalized];
  const listProblem = portListProblem(next);
  return listProblem ? { problem: listProblem } : { list: next, problem: '' };
}

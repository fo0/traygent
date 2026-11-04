import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import {
  MAX_PORTS,
  TOO_MANY,
  addPort,
  exposedIp,
  normalizePort,
  normalizePorts,
  parsePort,
  portLabel,
  portListProblem,
  presetPort,
} from './ports';

/*
 * docs/SPEC.md → Ports. The native side validates again and is the
 * authority; these cases are the grammar branches the panel mirrors.
 */
describe('port grammar and normalization', () => {
  it('binds a bare port to 127.0.0.1 on both sides', () => {
    expect(normalizePort('3000')).toBe('127.0.0.1:3000:3000');
    expect(normalizePort('  5173 ')).toBe('127.0.0.1:5173:5173');
    expect(presetPort(3000)).toBe('127.0.0.1:3000:3000');
  });
  it('binds HOST:CONTAINER to 127.0.0.1', () => {
    expect(normalizePort('8080:80')).toBe('127.0.0.1:8080:80');
  });
  it('keeps an explicit IPv4', () => {
    expect(normalizePort('0.0.0.0:3000:3000')).toBe('0.0.0.0:3000:3000');
    expect(normalizePort('192.168.1.20:8443:443')).toBe(
      '192.168.1.20:8443:443',
    );
  });
  it('takes ranges of equal length', () => {
    expect(normalizePort('8000-8002')).toBe('127.0.0.1:8000-8002:8000-8002');
    expect(normalizePort('9000-9001:3000-3001')).toBe(
      '127.0.0.1:9000-9001:3000-3001',
    );
    expect(normalizePort('3000-3000')).toBe('127.0.0.1:3000:3000');
    expect(normalizePort('9000-9002:3000-3001')).toBeNull();
    expect(normalizePort('3001-3000')).toBeNull();
  });
  it('keeps /udp and drops the default /tcp, in any case', () => {
    expect(normalizePort('53/udp')).toBe('127.0.0.1:53:53/udp');
    expect(normalizePort('53/UDP')).toBe('127.0.0.1:53:53/udp');
    expect(normalizePort('8080:80/TCP')).toBe('127.0.0.1:8080:80');
    expect(parsePort('3000').spec?.udp).toBe(false);
    expect(normalizePort('3000/sctp')).toBeNull();
    expect(normalizePort('3000/')).toBeNull();
    expect(normalizePort('3000/udp/tcp')).toBeNull();
  });
  it('refuses numbers outside 1–65535 and malformed entries', () => {
    for (const raw of [
      '',
      '0',
      '65536',
      '123456',
      '-1',
      '30a0',
      ':3000',
      '3000:',
      '1.2.3:80:80',
      '256.0.0.1:80:80',
      '01.2.3.4:80:80',
      'localhost:80:80',
      '1:2:3:4',
      '[::1]:80:80',
      '1-2-3',
      '3000 3001',
    ])
      expect(normalizePort(raw), JSON.stringify(raw)).toBeNull();
    expect(normalizePort('65535')).toBe('127.0.0.1:65535:65535');
    expect(normalizePort('1')).toBe('127.0.0.1:1:1');
    // Leading zeros are still the number, as natively.
    expect(normalizePort('03000')).toBe('127.0.0.1:3000:3000');
  });
  it('names the reason in German, like the native side', () => {
    const problem = (raw: string) => parsePort(raw).problem;
    expect(problem('3000')).toBe('');
    expect(problem('abc')).toBe('Port „abc“ ist ungültig.');
    expect(problem(' 3001-3000 ')).toBe('Port „3001-3000“ ist ungültig.');
    expect(problem('70000')).toBe('Port „70000“ liegt außerhalb von 1–65535.');
    expect(problem('8080:0')).toBe(
      'Port „8080:0“ liegt außerhalb von 1–65535.',
    );
    expect(problem('localhost:80:80')).toBe(
      'IP-Adresse in „localhost:80:80“ ist ungültig (nur IPv4).',
    );
    expect(problem('8000-8002:80-81')).toBe(
      'Host- und Container-Bereich in „8000-8002:80-81“ müssen gleich lang sein.',
    );
    expect(problem('8000-8100')).toBe(
      'Bereich in „8000-8100“ umfasst mehr als 100 Ports.',
    );
    expect(problem('8000-8099')).toBe('');
    // Long input is refused and shown cut to 32 characters.
    expect(problem('1'.repeat(70))).toBe(
      `Port „${'1'.repeat(32)}…“ ist ungültig.`,
    );
    expect(problem('30\u000700')).toBe('Port „3000“ ist ungültig.');
  });
  it('labels a chip with the host side', () => {
    expect(portLabel('127.0.0.1:8080:80')).toBe('8080');
    expect(portLabel('127.0.0.1:8000-8001:8000-8001/udp')).toBe(
      '8000-8001/udp',
    );
  });
  it('names the IP of an entry the network reaches', () => {
    expect(exposedIp('127.0.0.1:3000:3000')).toBeNull();
    expect(exposedIp('3000')).toBeNull();
    expect(exposedIp('abc')).toBeNull();
    expect(exposedIp('0.0.0.0:3000:3000')).toBe('0.0.0.0');
    expect(exposedIp('127.0.0.2:3000:3000')).toBe('127.0.0.2');
    expect(portLabel('0.0.0.0:3000:80')).toBe('0.0.0.0:3000');
    expect(portLabel('192.168.1.20:53:53/udp')).toBe('192.168.1.20:53/udp');
  });
});

describe('port lists', () => {
  it('accepts distinct host ports and the same port on both protocols', () => {
    expect(portListProblem([])).toBe('');
    expect(
      portListProblem(['127.0.0.1:3000:3000', '127.0.0.1:53:53/udp']),
    ).toBe('');
    expect(
      portListProblem(['127.0.0.1:53:53/tcp', '127.0.0.1:53:53/udp']),
    ).toBe('');
  });
  it('refuses a host port used twice on one protocol, ranges included', () => {
    expect(portListProblem(['127.0.0.1:3000:3000', '0.0.0.0:3000:80'])).toBe(
      'Host-Port 3000 ist doppelt belegt.',
    );
    expect(
      portListProblem(['127.0.0.1:8000-8005:8000-8005', '127.0.0.1:8003:80']),
    ).toBe('Host-Port 8003 ist doppelt belegt.');
    expect(
      portListProblem(['127.0.0.1:3000:3000', '127.0.0.1:3000:3000/tcp']),
    ).toBe('Host-Port 3000 ist doppelt belegt.');
    expect(
      portListProblem(['127.0.0.1:53:53/udp', '0.0.0.0:50-60:50-60/udp']),
    ).toBe('Host-Port 53/udp ist doppelt belegt.');
  });
  it('names the first invalid entry', () => {
    expect(portListProblem(['127.0.0.1:3000:3000', ' 3o00 '])).toBe(
      'Port „3o00“ ist ungültig.',
    );
  });
  it('drops blank entries and allows at most 20, checked first', () => {
    expect(portListProblem(['  ', ''])).toBe('');
    const list = (count: number) =>
      Array.from({ length: count }, (_, i) => presetPort(10000 + i));
    expect(portListProblem(list(MAX_PORTS))).toBe('');
    expect(portListProblem(list(MAX_PORTS + 1))).toBe(TOO_MANY);
    expect(portListProblem([...list(MAX_PORTS), 'x'])).toBe(TOO_MANY);
    expect(TOO_MANY).toBe('Höchstens 20 Port-Angaben erlaubt.');
  });
  it('adds a typed entry normalized, or says why not', () => {
    expect(addPort([], '8080:80')).toEqual({
      list: ['127.0.0.1:8080:80'],
      problem: '',
    });
    expect(addPort(['127.0.0.1:8080:80'], '8080:80')).toEqual({
      list: ['127.0.0.1:8080:80'],
      problem: '',
    });
    expect(addPort([], 'abc')).toEqual({ problem: 'Port „abc“ ist ungültig.' });
    expect(addPort([presetPort(3000)], '3000:80')).toEqual({
      problem: 'Host-Port 3000 ist doppelt belegt.',
    });
    const full = Array.from({ length: MAX_PORTS }, (_, i) =>
      presetPort(10000 + i),
    );
    expect(addPort(full, '3000')).toEqual({ problem: TOO_MANY });
  });
});

type Expected<T> = { normalized: T; error?: undefined } | { error: string };
interface Vectors {
  ports: ({ input: string } & Expected<string>)[];
  lists: ({ list: string[] } & Expected<string[]>)[];
}
/** fixtures/port-vectors.json, checked natively by src-tauri/src/ports/tests.rs too. */
const vectors: Vectors = JSON.parse(
  readFileSync(join(process.cwd(), 'fixtures/port-vectors.json'), 'utf8'),
);

describe('shared port vectors (same as the native side)', () => {
  it('has a vector for every branch', () => {
    expect(vectors.ports.length).toBeGreaterThanOrEqual(40);
  });
  it.each(
    vectors.ports.map((vector) => [JSON.stringify(vector.input), vector]),
  )('entry %s', (_, vector) => {
    if (vector.error === undefined) {
      expect(normalizePort(vector.input)).toBe(vector.normalized);
      expect(addPort([], vector.input)).toEqual({
        list: [vector.normalized],
        problem: '',
      });
    } else {
      expect(parsePort(vector.input).problem).toBe(vector.error);
      expect(addPort([], vector.input)).toEqual({ problem: vector.error });
    }
  });
  it.each(vectors.lists.map((vector) => [JSON.stringify(vector.list), vector]))(
    'list %s',
    (_, vector) => {
      if (vector.error === undefined) {
        expect(portListProblem(vector.list)).toBe('');
        expect(normalizePorts(vector.list)).toEqual(vector.normalized);
      } else {
        expect(portListProblem(vector.list)).toBe(vector.error);
      }
    },
  );
});

import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  mkdtempSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
  rmSync,
  copyFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import {
  versionFromTag,
  releasePlan,
  validatePlan,
  assertNewRelease,
  stampVersions,
  assertVersions,
  releaseNotes,
} from './release.mjs';
const sourceSha = 'a'.repeat(40);
const timestamp = '2026-10-31T15:33:04Z';
const plan = releasePlan(timestamp, sourceSha);
/** @param {import('node:test').TestContext} t */
function workspace(t) {
  const directory = mkdtempSync(path.join(tmpdir(), 'traygent-release-'));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  mkdirSync(path.join(directory, 'src-tauri'));
  writeFileSync(
    path.join(directory, 'package.json'),
    JSON.stringify({ name: 'traygent', version: '0.1.0', type: 'module' }),
  );
  writeFileSync(
    path.join(directory, 'src-tauri/tauri.conf.json'),
    JSON.stringify({ version: '0.1.0', identifier: 'io.github.fo0.traygent' }),
  );
  writeFileSync(
    path.join(directory, 'src-tauri/Cargo.toml'),
    '[package]\nname = "traygent"\nversion = "0.1.0"\n\n[dependencies]\nserde = { version = "1" }\n',
  );
  writeFileSync(
    path.join(directory, 'src-tauri/Cargo.lock'),
    'version = 4\n\n[[package]]\nname = "dependency"\nversion = "1.2.3"\n\n[[package]]\nname = "traygent"\nversion = "0.1.0"\ndependencies = ["dependency"]\n',
  );
  return directory;
}
test('stable canonical tags reject input that is unsafe or not semver', () => {
  assert.equal(versionFromTag('v0.1.0'), '0.1.0');
  assert.equal(versionFromTag('v2026.1031.1533'), '2026.1031.1533');
  for (const tag of [
    'main',
    'v0.1.0;echo unsafe',
    'v1.0.0-beta',
    '../main',
    'v2026.0101.0001',
  ])
    assert.throws(() => versionFromTag(tag));
});
test('one stable UTC timestamp produces version, tag and Berlin release title', () => {
  assert.equal(plan.version, '2026.1031.1533');
  assert.equal(plan.tag, 'v2026.1031.1533');
  assert.equal(plan.title, 'Traygent 2026-10-31 16:33 (Berlin, GMT+1)');
  assert.equal(plan.source_sha, sourceSha);
  assert.deepEqual(releasePlan(timestamp, sourceSha), plan);
});
test('midnight, leading zero fields, leap day and year rollover remain Windows-compatible', () => {
  /** @type {[date: string, version: string][]} */
  const cases = [
    ['2026-01-01T00:00:00Z', '2026.101.0'],
    ['2026-01-02T00:05:00Z', '2026.102.5'],
    ['2028-02-29T23:59:59Z', '2028.229.2359'],
    ['2026-12-31T23:59:59Z', '2026.1231.2359'],
    ['2027-01-01T00:00:00Z', '2027.101.0'],
  ];
  for (const [date, version] of cases) {
    const result = releasePlan(date, sourceSha);
    assert.equal(result.version, version);
    assert.ok(version.split('.').every((part) => Number(part) <= 65535));
  }
});
test('Berlin daylight-saving clock rollback does not repeat or downgrade the version', () => {
  const before = releasePlan('2026-10-25T00:59:00Z', sourceSha);
  const after = releasePlan('2026-10-25T01:00:00Z', sourceSha);
  assert.match(before.title, /02:59.*GMT\+2/);
  assert.match(after.title, /02:00.*GMT\+1/);
  assert.equal(before.version, '2026.1025.59');
  assert.equal(after.version, '2026.1025.100');
});
test('invalid, ambiguous or tampered release plans fail closed', () => {
  for (const date of [
    '2026-02-29T12:00:00Z',
    '2026-04-31T12:00:00Z',
    '2026-10-31T15:33:00',
    '2026-10-31T16:33:00+01:00',
    '1999-01-01T00:00:00Z',
  ])
    assert.throws(() => releasePlan(date, sourceSha));
  assert.throws(() => releasePlan(timestamp, 'main'));
  for (const change of [
    { version: '0.1.0' },
    { tag: 'v0.1.0' },
    { title: 'Other release' },
    { created_at: '2026-10-31T15:34:04.000Z' },
  ])
    assert.throws(() => validatePlan({ ...plan, ...change }));
});
test('retry, same-minute collision and publishing an older run cannot replace latest', () => {
  const older = { tag_name: 'v0.1.0', draft: false, prerelease: false };
  assert.doesNotThrow(() => assertNewRelease(plan, [[older]]));
  for (const tag of [plan.tag, 'v2026.1031.1534', 'v2027.101.0'])
    assert.throws(() => assertNewRelease(plan, [{ ...older, tag_name: tag }]));
  assert.throws(() =>
    assertNewRelease(plan, [{ ...older, tag_name: plan.tag, draft: true }]),
  );
  assert.throws(() => assertNewRelease(plan, { error: 'Permission denied' }));
});
test('stamp keeps all four app/lockfile versions aligned without modifying dependencies', (t) => {
  const directory = workspace(t);
  const tauri =
    '{\n  "version": "0.1.0",\n  "identifier": "io.github.fo0.traygent",\n  "bundle": { "targets": ["nsis"] }\n}\n';
  writeFileSync(path.join(directory, 'src-tauri/tauri.conf.json'), tauri);
  stampVersions(directory, plan);
  assertVersions(directory, plan);
  const cargo = readFileSync(
    path.join(directory, 'src-tauri/Cargo.toml'),
    'utf8',
  );
  const lock = readFileSync(
    path.join(directory, 'src-tauri/Cargo.lock'),
    'utf8',
  );
  assert.match(cargo, /serde = \{ version = "1" \}/);
  assert.match(lock, /name = "dependency"\nversion = "1.2.3"/);
  assert.equal(
    JSON.parse(
      readFileSync(path.join(directory, 'src-tauri/tauri.conf.json'), 'utf8'),
    ).identifier,
    'io.github.fo0.traygent',
  );
  assert.equal(
    readFileSync(path.join(directory, 'src-tauri/tauri.conf.json'), 'utf8'),
    tauri.replace('0.1.0', plan.version),
  );
  writeFileSync(
    path.join(directory, 'package.json'),
    JSON.stringify({ name: 'traygent', version: '0.1.0' }),
  );
  assert.throws(() => assertVersions(directory, plan));
});
test('missing/ambiguous Cargo lock entry fails before any stamp writes', (t) => {
  const directory = workspace(t);
  const pkg = readFileSync(path.join(directory, 'package.json'), 'utf8');
  for (const lock of [
    '',
    '[[package]]\nname = "traygent"\nversion = "0.1.0"\n\n[[package]]\nname = "traygent"\nversion = "0.2.0"\n',
  ]) {
    writeFileSync(path.join(directory, 'src-tauri/Cargo.lock'), lock);
    assert.throws(() => stampVersions(directory, plan));
    assert.equal(
      readFileSync(path.join(directory, 'package.json'), 'utf8'),
      pkg,
    );
  }
});
test('automatic changelog retains generated content and source provenance', () => {
  const notes = releaseNotes(
    plan,
    '## Changes\n- Improve workflow search (#4)\n',
  );
  assert.match(notes, /2026-10-31 16:33/);
  assert.match(notes, /Improve workflow search \(#4\)/);
  assert.ok(notes.includes(sourceSha));
  // In-app updates (ADR-0004): the notes never send users to a manual install.
  assert.match(notes, /Installed builds update in the app/);
  assert.doesNotMatch(notes, /install it manually/);
  assert.match(
    releaseNotes(plan, '  '),
    /No additional release notes were generated/,
  );
});
test('CLI generates versions and release notes without signing configuration', (t) => {
  const directory = workspace(t);
  mkdirSync(path.join(directory, 'scripts'));
  const script = path.join(directory, 'scripts/release.mjs');
  copyFileSync(
    fileURLToPath(new URL('./release.mjs', import.meta.url)),
    script,
  );
  const metadata = path.join(directory, 'release-meta.json');
  const generated = path.join(directory, 'generated.md');
  const notes = path.join(directory, 'notes.md');
  /** @param {string[]} args */
  const call = (args) =>
    spawnSync(process.execPath, [script, ...args], { encoding: 'utf8' });
  assert.equal(call(['plan', timestamp, sourceSha, metadata]).status, 0);
  writeFileSync(generated, '## Changes\n- A generated change');
  const result = call(['prepare', metadata, generated, notes]);
  assert.equal(result.status, 0, result.stderr);
  assertVersions(directory, plan);
  assert.match(readFileSync(notes, 'utf8'), /not Authenticode signed/);
  assert.doesNotMatch(readFileSync(notes, 'utf8'), /cryptographically signed/);
});

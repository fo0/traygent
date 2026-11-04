import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { updaterConfig, updaterManifest } from './updater.mjs';

/** @param {string} version */
const signature = (version) =>
  Buffer.from(
    `untrusted comment: test fixture\nnot-a-real-signature\ntrusted comment: timestamp:1\tfile:installer.exe\tversion:${version}\nnot-a-real-global-signature\n`,
  ).toString('base64');

test('release feed accepts version-bound artifacts produced by the pinned Tauri CLI', (t) => {
  const directory = mkdtempSync(path.join(tmpdir(), 'traygent-updater-test-'));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  const cli = fileURLToPath(
    new URL('../node_modules/@tauri-apps/cli/tauri.js', import.meta.url),
  );
  const key = path.join(directory, 'test-only.key');
  const file = path.join(directory, 'test-only.exe');
  // Disposable test material; output is captured so no private key enters logs.
  /** @param {string[]} args */
  const call = (...args) =>
    spawnSync(process.execPath, [cli, ...args], { encoding: 'utf8' });
  assert.equal(
    call('signer', 'generate', '--ci', '-p', 'test-only', '-w', key).status,
    0,
    'Test key generation failed',
  );
  const config = updaterConfig(readFileSync(`${key}.pub`, 'utf8'), {
    bundle: { createUpdaterArtifacts: true },
  });
  assert.ok(config.plugins.updater.pubkey);
  writeFileSync(file, 'Synthetic update package, never installed.');
  assert.equal(
    call(
      'signer',
      'sign',
      '-f',
      key,
      '-p',
      'test-only',
      '--app-version',
      '0.2.0',
      file,
    ).status,
    0,
    'Test artifact signing failed',
  );
  const sig = readFileSync(`${file}.sig`, 'utf8');
  assert.equal(
    updaterManifest('0.2.0', sig, 'Test release', '2026-11-01T00:00:00Z')
      .platforms['windows-x86_64'].signature,
    sig.trim(),
  );
  assert.throws(() =>
    updaterManifest('0.3.0', sig, '', '2026-11-01T00:00:00Z'),
  );
});

test('feed uses the exact public release and preserves changelog and version-bound signature', () => {
  const sig = signature('2026.1101.15');
  const feed = updaterManifest(
    '2026.1101.15',
    sig,
    '## Changes\n- Faster updates',
    '2026-11-01T00:15:00Z',
  );
  assert.equal(feed.version, '2026.1101.15');
  assert.equal(feed.notes, '## Changes\n- Faster updates');
  assert.equal(feed.pub_date, '2026-11-01T00:15:00.000Z');
  assert.deepEqual(feed.platforms, {
    'windows-x86_64': {
      signature: sig,
      url: 'https://github.com/fo0/traygent/releases/download/v2026.1101.15/Traygent_2026.1101.15_x64-setup.exe',
    },
  });
});

test('missing or differently versioned signatures cannot create an update feed', () => {
  for (const sig of ['', 'not a signature', signature('0.1.0')])
    assert.throws(() =>
      updaterManifest('0.2.0', sig, '', '2026-11-01T00:00:00Z'),
    );
  assert.throws(() =>
    updaterManifest(
      '0.2.0-beta',
      signature('0.2.0-beta'),
      '',
      '2026-11-01T00:00:00Z',
    ),
  );
  assert.throws(() =>
    updaterManifest('0.2.0', signature('0.2.0'), '', 'invalid'),
  );
});

test('release configuration rejects missing signing configuration and includes only a public key', () => {
  assert.throws(() => updaterConfig('', {}));
  assert.throws(() => updaterConfig('/path/to/key.pub', {}));
  const publicKey = Buffer.from(
    `untrusted comment: test public key\n${Buffer.alloc(42).toString('base64')}\n`,
  ).toString('base64');
  const result = updaterConfig(publicKey, {
    bundle: { createUpdaterArtifacts: true },
  });
  assert.equal(result.plugins.updater.pubkey, publicKey);
  assert.equal(result.bundle.createUpdaterArtifacts, true);
  assert.deepEqual(Object.keys(result), ['bundle', 'plugins']);
});

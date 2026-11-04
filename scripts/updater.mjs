import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { versionFromTag } from './release.mjs';

/**
 * @template {{ plugins?: { updater?: object }; [key: string]: unknown }} T
 * @param {string | undefined} publicKey
 * @param {T} releaseConfig
 */
export function updaterConfig(publicKey, releaseConfig) {
  if (!publicKey?.trim())
    throw new Error(
      'A public key is required only for separately configured signed updater artifacts',
    );
  const decoded = Buffer.from(publicKey.trim(), 'base64').toString('utf8');
  const lines = decoded.trim().split(/\r?\n/);
  if (
    lines.length !== 2 ||
    !lines[0]?.startsWith('untrusted comment:') ||
    Buffer.from(lines[1] ?? '', 'base64').length !== 42
  )
    throw new Error(
      'Expected the contents of the Tauri .pub file, not its path',
    );
  return {
    ...releaseConfig,
    plugins: {
      ...releaseConfig.plugins,
      updater: { ...releaseConfig.plugins?.updater, pubkey: publicKey.trim() },
    },
  };
}

/**
 * @param {string} version
 * @param {string} signature
 * @param {string} notes
 * @param {string} date
 */
export function updaterManifest(version, signature, notes, date) {
  versionFromTag(`v${version}`);
  const published = new Date(date);
  if (!Number.isFinite(published.getTime()))
    throw new Error('Invalid publication date');
  // Version is covered by minisign's trusted comment, not just the unsigned JSON.
  const decoded = Buffer.from(signature.trim(), 'base64').toString('utf8');
  if (
    !decoded
      .split(/\r?\n/)
      .some(
        (line) =>
          line.startsWith('trusted comment:') &&
          line.endsWith(`\tversion:${version}`),
      )
  )
    throw new Error('Updater signature is missing its matching signed version');
  return {
    version,
    notes,
    pub_date: published.toISOString(),
    platforms: {
      'windows-x86_64': {
        signature: signature.trim(),
        url: `https://github.com/fo0/traygent/releases/download/v${version}/Traygent_${version}_x64-setup.exe`,
      },
    },
  };
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [command, ...args] = process.argv.slice(2);
  /** @param {number} index */
  const arg = (index) => {
    const value = args[index];
    if (value === undefined)
      throw new Error('Missing updater command argument');
    return value;
  };
  if (command === 'config') {
    const config = updaterConfig(
      process.env.TRAYGENT_UPDATER_PUBLIC_KEY,
      JSON.parse(readFileSync('src-tauri/tauri.release.conf.json', 'utf8')),
    );
    writeFileSync(arg(0), JSON.stringify(config, null, 2) + '\n');
  } else if (command === 'manifest') {
    writeFileSync(
      arg(4),
      JSON.stringify(
        updaterManifest(
          arg(0),
          readFileSync(arg(1), 'utf8'),
          readFileSync(arg(2), 'utf8'),
          arg(3),
        ),
        null,
        2,
      ) + '\n',
    );
  } else {
    throw new Error('Unknown updater command');
  }
}

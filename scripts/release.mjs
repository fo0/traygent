import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
/**
 * @typedef {{
 *   version: string;
 *   tag: string;
 *   title: string;
 *   created_at: string;
 *   source_sha: string;
 * }} ReleasePlan
 */
/** @param {string} tag */
export function versionFromTag(tag) {
  if (!/^v(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)$/.test(tag))
    throw new Error('Expected a stable vX.Y.Z tag');
  return tag.slice(1);
}
/**
 * @param {string} timestamp
 * @param {string} sourceSha
 * @returns {ReleasePlan}
 */
export function releasePlan(timestamp, sourceSha) {
  if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/.test(timestamp))
    throw new Error('Expected an explicit UTC release timestamp');
  const date = new Date(timestamp);
  if (
    !Number.isFinite(date.getTime()) ||
    date.toISOString().slice(0, 19) !== timestamp.slice(0, 19) ||
    date.getUTCFullYear() < 2000
  )
    throw new Error('Invalid release timestamp');
  if (!/^[a-f0-9]{40}$/.test(sourceSha))
    throw new Error('Expected the exact source commit SHA');
  // Three numeric components keep Tauri semver ordering and NSIS WORD limits.
  // UTC avoids ambiguous/backwards versions during Berlin's autumn clock change.
  const version = `${date.getUTCFullYear()}.${(date.getUTCMonth() + 1) * 100 + date.getUTCDate()}.${date.getUTCHours() * 100 + date.getUTCMinutes()}`;
  const parts = Object.fromEntries(
    new Intl.DateTimeFormat('en-GB', {
      timeZone: 'Europe/Berlin',
      year: 'numeric',
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
      hourCycle: 'h23',
      timeZoneName: 'shortOffset',
    })
      .formatToParts(date)
      .map(({ type, value }) => [type, value]),
  );
  return {
    version,
    tag: `v${version}`,
    title: `Traygent ${parts.year}-${parts.month}-${parts.day} ${parts.hour}:${parts.minute} (Berlin, ${parts.timeZoneName})`,
    created_at: date.toISOString(),
    source_sha: sourceSha,
  };
}
/** @param {ReleasePlan} plan */
export function validatePlan(plan) {
  const expected = releasePlan(plan.created_at, plan.source_sha);
  for (const key of /** @type {(keyof ReleasePlan)[]} */ (
    Object.keys(expected)
  )) {
    if (plan[key] !== expected[key]) throw new Error('Release plan mismatch');
  }
  return plan;
}
/**
 * @param {ReleasePlan} plan
 * @param {unknown} releases
 */
export function assertNewRelease(plan, releases) {
  validatePlan(plan);
  if (!Array.isArray(releases)) throw new Error('Release history unavailable');
  const candidate = plan.version.split('.').map(BigInt);
  for (const release of releases.flat()) {
    if (!release || typeof release.tag_name !== 'string')
      throw new Error('Invalid release history');
    if (release.tag_name === plan.tag)
      throw new Error(
        'Release already exists for this minute; inspect it before starting a new run',
      );
    if (release.draft || release.prerelease) continue;
    /** @type {bigint[]} */
    let previous;
    try {
      previous = versionFromTag(release.tag_name).split('.').map(BigInt);
    } catch {
      continue;
    }
    const different = previous.findIndex(
      (part, index) => part !== candidate[index],
    );
    const older = previous[different];
    const newer = candidate[different];
    if (
      different === -1 ||
      (older !== undefined && newer !== undefined && older > newer)
    )
      throw new Error('Release would not be newer than the published versions');
  }
}
/** @param {string} directory */
function appVersions(directory) {
  const pkgText = readFileSync(path.join(directory, 'package.json'), 'utf8');
  const tauriText = readFileSync(
    path.join(directory, 'src-tauri/tauri.conf.json'),
    'utf8',
  );
  const pkg = JSON.parse(pkgText);
  const tauri = JSON.parse(tauriText);
  const cargo = readFileSync(
    path.join(directory, 'src-tauri/Cargo.toml'),
    'utf8',
  );
  const lock = readFileSync(
    path.join(directory, 'src-tauri/Cargo.lock'),
    'utf8',
  );
  return { pkg, tauri, pkgText, tauriText, cargo, lock };
}
/**
 * @param {string} directory
 * @param {ReleasePlan} plan
 */
export function stampVersions(directory, plan) {
  validatePlan(plan);
  const { pkg, pkgText, tauriText, cargo, lock } = appVersions(directory);
  if (pkg.name !== 'traygent') throw new Error('Expected the Traygent package');
  const manifestPattern = /^(\[package\][\s\S]*?^version\s*=\s*)"[^"]+"/m;
  const lockPattern =
    /^(\[\[package\]\]\r?\nname = "traygent"\r?\nversion = )"[^"]+"/gm;
  if (
    !manifestPattern.test(cargo) ||
    [...lock.matchAll(lockPattern)].length !== 1
  )
    throw new Error(
      'Traygent Cargo package/lockfile entry missing or ambiguous',
    );
  /** @param {string} text */
  const versionJson = (text) => {
    // Preserve formatting: pnpm verify lints these files after the CI stamp.
    const changed = text.replace(
      /("version"\s*:\s*)"[^"]*"/,
      `$1"${plan.version}"`,
    );
    const expected = { ...JSON.parse(text), version: plan.version };
    if (JSON.stringify(JSON.parse(changed)) !== JSON.stringify(expected))
      throw new Error('Root JSON app version not found');
    return changed;
  };
  // Validate all inputs before changing any file. Dependency versions stay intact.
  /** @type {[file: string, contents: string][]} */
  const changes = [
    ['package.json', versionJson(pkgText)],
    ['src-tauri/tauri.conf.json', versionJson(tauriText)],
    [
      'src-tauri/Cargo.toml',
      cargo.replace(manifestPattern, `$1"${plan.version}"`),
    ],
    ['src-tauri/Cargo.lock', lock.replace(lockPattern, `$1"${plan.version}"`)],
  ];
  for (const [file, contents] of changes)
    writeFileSync(path.join(directory, file), contents);
}
/**
 * @param {string} directory
 * @param {ReleasePlan} plan
 */
export function assertVersions(directory, plan) {
  validatePlan(plan);
  const { pkg, tauri, cargo, lock } = appVersions(directory);
  const rustVersion = /^\[package\][\s\S]*?^version\s*=\s*"([^"]+)"/m.exec(
    cargo,
  )?.[1];
  const lockVersion =
    /^\[\[package\]\]\r?\nname = "traygent"\r?\nversion = "([^"]+)"/m.exec(
      lock,
    )?.[1];
  if (
    [pkg.version, tauri.version, rustVersion, lockVersion].some(
      (v) => v !== plan.version,
    )
  )
    throw new Error('Release plan and app versions differ');
}
/**
 * @param {ReleasePlan} plan
 * @param {string} generatedNotes
 */
export function releaseNotes(plan, generatedNotes) {
  validatePlan(plan);
  const changelog =
    generatedNotes.trim() || 'No additional release notes were generated.';
  return `${plan.title}\n\nVersion: ${plan.version}\nSource: ${plan.source_sha}\n\n${changelog}\n\nWindows installer: not Authenticode signed. Installed builds update in the app; for a first install, download and run the EXE. This release does not include signed Tauri updater artifacts.\n`;
}
if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [command, ...args] = process.argv.slice(2);
  /** @param {number} index */
  const arg = (index) => {
    const value = args[index];
    if (value === undefined)
      throw new Error('Missing release command argument');
    return value;
  };
  if (command === 'plan') {
    writeFileSync(
      arg(2),
      JSON.stringify(releasePlan(arg(0), arg(1)), null, 2) + '\n',
    );
    process.exit(0);
  }
  const plan = validatePlan(JSON.parse(readFileSync(arg(0), 'utf8')));
  if (command === 'check-history') {
    assertNewRelease(plan, JSON.parse(readFileSync(arg(1), 'utf8')));
  } else if (command === 'prepare') {
    const notes = releaseNotes(plan, readFileSync(arg(1), 'utf8'));
    stampVersions(root, plan);
    assertVersions(root, plan);
    writeFileSync(arg(2), notes);
  } else throw new Error('Unknown release command');
}

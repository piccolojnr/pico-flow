import { readFileSync, appendFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

export function validateTag(tag, version) {
  const pattern = /^v(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-(alpha|beta|rc)\.(0|[1-9]\d*))?$/;
  if (!pattern.test(tag)) throw new Error('Release tag must be vMAJOR.MINOR.PATCH or vMAJOR.MINOR.PATCH-{alpha,beta,rc}.N');
  if (tag.slice(1) !== version) throw new Error(`Tag ${tag} does not match app version ${version}`);
  return { tag, version, prerelease: tag.includes('-') };
}
export function projectVersion(root = '.') {
  const json = path => JSON.parse(readFileSync(resolve(root, path), 'utf8'));
  const tauri = json('src-tauri/tauri.conf.json').version;
  const npm = json('package.json').version;
  const npmLock = json('package-lock.json');
  const cargo = readFileSync(resolve(root, 'src-tauri/Cargo.toml'), 'utf8').split('[package]')[1]?.split(/\n\[/)[0].match(/^version\s*=\s*"([^"]+)"/m)?.[1];
  const lock = readFileSync(resolve(root, 'src-tauri/Cargo.lock'), 'utf8').split('[[package]]').find(p => /^name = "flow-linux"$/m.test(p))?.match(/^version = "([^"]+)"/m)?.[1];
  if (![npm, npmLock.version, npmLock.packages?.[''].version, cargo, lock].every(v => v === tauri)) throw new Error('Version mismatch between Tauri, Cargo, npm, and their lockfiles');
  validateTag(`v${tauri}`, tauri);
  return tauri;
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const version = projectVersion();
  const tag = process.env.RELEASE_TAG;
  const info = tag ? validateTag(tag, version) : {version};
  console.log(JSON.stringify(info));
  if (process.env.GITHUB_OUTPUT) appendFileSync(process.env.GITHUB_OUTPUT, Object.entries(info).map(([k,v])=>`${k}=${v}\n`).join(''));
}

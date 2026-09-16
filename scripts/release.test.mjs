import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { projectVersion, validateTag } from './release.mjs';

test('stable and prerelease tags are classified correctly', () => {
  assert.deepEqual(validateTag('v1.2.3','1.2.3'), {tag:'v1.2.3',version:'1.2.3',prerelease:false});
  for (const stage of ['alpha','beta','rc']) assert.equal(validateTag(`v1.2.3-${stage}.1`,`1.2.3-${stage}.1`).prerelease, true);
});
test('rejects malformed or mismatched release tags', () => {
  for (const tag of ['1.2.3','v01.2.3','v1.2','v1.2.3+build','v1.2.3-rc.01','v1.2.3\ncontents=write','v1.2.4']) assert.throws(() => validateTag(tag,'1.2.3'));
});
test('all version files must agree, including lockfiles', () => {
  const dir = mkdtempSync(join(tmpdir(),'flow-version-'));
  try {
    mkdirSync(join(dir,'src-tauri'));
    for (const path of ['package.json','package-lock.json','src-tauri/tauri.conf.json','src-tauri/Cargo.toml','src-tauri/Cargo.lock']) writeFileSync(join(dir,path),readFileSync(path));
    assert.equal(projectVersion(dir), projectVersion());
    const path=join(dir,'package-lock.json');const lock=JSON.parse(readFileSync(path));lock.packages[''].version='99.0.0';writeFileSync(path,JSON.stringify(lock));
    assert.throws(()=>projectVersion(dir),/Version mismatch/);
  } finally {rmSync(dir,{recursive:true,force:true});}
});

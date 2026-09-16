import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { projectVersion, validateTag } from './release.mjs';
const release=validateTag(process.env.RELEASE_TAG,projectVersion());
const targets={ 'x86_64-unknown-linux-gnu':['.deb','.AppImage'], 'x86_64-pc-windows-msvc':['.exe'] };
const expected=Object.entries(targets).flatMap(([target,extensions])=>extensions.map(ext=>`flow-${release.tag}-${target}${ext}`));
const files=readdirSync('release-assets');
const sums=[];
for(const name of expected){
 if(!files.includes(name))throw new Error(`Missing release asset: ${name}`);
 sums.push(`${createHash('sha256').update(readFileSync(join('release-assets',name))).digest('hex')}  ${name}`);
}
writeFileSync('release-assets/SHA256SUMS',sums.join('\n')+'\n');
const gh=(args)=>{const r=spawnSync('gh',args,{stdio:'inherit'});if(r.status!==0)throw new Error(`gh ${args[0]} failed`);};
const probe=spawnSync('gh',['release','view',release.tag,'--json','isDraft'],{encoding:'utf8'});
if(probe.status===0 && !JSON.parse(probe.stdout).isDraft)throw new Error('Refusing to replace an already published release');
if(probe.status!==0)gh(['release','create',release.tag,'--verify-tag','--draft','--title',`Flow ${release.version}`,'--generate-notes',...(release.prerelease?['--prerelease']:[])]);
gh(['release','upload',release.tag,...expected.map(name=>join('release-assets',name)),'release-assets/SHA256SUMS','--clobber']);
// Publish only after every matrix artifact is present and uploaded.
gh(['release','edit',release.tag,'--draft=false',`--prerelease=${release.prerelease}`,`--latest=${!release.prerelease}`]);

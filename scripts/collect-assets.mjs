import { mkdirSync, readdirSync, copyFileSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { createHash } from 'node:crypto';
import { projectVersion } from './release.mjs';
const target = process.env.BUILD_TARGET;
const expected = (process.env.BUILD_BUNDLES || '').split(',').map(type=>({deb:'.deb',appimage:'.AppImage',nsis:'.exe',dmg:'.dmg'}[type]));
if(!target || expected.some(v=>!v))throw new Error('Unknown build target or bundle type');
const root=join('src-tauri','target',target,'release','bundle');
const walk=path=>readdirSync(path,{withFileTypes:true}).flatMap(entry=>entry.isDirectory()?walk(join(path,entry.name)):[join(path,entry.name)]);
const files=walk(root);mkdirSync('release-assets',{recursive:true});
const sums=[];
for(const ext of expected){
  const found=files.filter(path=>path.endsWith(ext));
  if(found.length!==1)throw new Error(`Expected exactly one ${ext} bundle, found ${found.length}`);
  const name=`flow-v${projectVersion()}-${target}${ext}`;
  copyFileSync(found[0],join('release-assets',name));
  sums.push(`${createHash('sha256').update(readFileSync(found[0])).digest('hex')}  ${name}`);
}
writeFileSync(join('release-assets',`${target}.sha256`),sums.join('\n')+'\n');
console.log(sums.join('\n'));

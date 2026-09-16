import { readFileSync, existsSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { spawnSync } from 'node:child_process';
for (const file of ['ui/index.html','ui/overlay.html']) {
  const html=readFileSync(file,'utf8');
  for (const [,attribute] of html.matchAll(/(?:src|href)="([^"#]+)"/g)) {
    if (/^(?:https?:|data:)/.test(attribute)) continue;
    if (!existsSync(resolve(dirname(file),attribute))) throw new Error(`Missing asset: ${attribute}`);
  }
  for (const [,script] of html.matchAll(/<script\s+type="module">([\s\S]*?)<\/script>/g)) {
    const result=spawnSync(process.execPath,['--check','--input-type=module'],{input:script,encoding:'utf8'});
    if(result.status!==0)throw new Error(result.stderr);
  }
}
const check=spawnSync(process.execPath,['--check','ui/app.js'],{encoding:'utf8'});
if(check.status!==0)throw new Error(check.stderr);
console.log('UI scripts and local asset references verified.');

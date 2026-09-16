import { spawnSync } from 'node:child_process';
const environment = {...process.env};
// An unset signing option is different from an empty credential to the bundler.
for(const key of Object.keys(environment)) if ((key.startsWith('APPLE_') || key.startsWith('WINDOWS_CERTIFICATE')) && !environment[key]) delete environment[key];
// Tauri CLI does not expose Cargo's --locked flag. The CI job runs locked
// checks before packaging; passing it here makes the CLI reject the build.
const args = ['node_modules/@tauri-apps/cli/tauri.js','build','--ci','--target',process.env.BUILD_TARGET,'--bundles',process.env.BUILD_BUNDLES];
if(args.some(v=>v===undefined))throw new Error('BUILD_TARGET and BUILD_BUNDLES must be set');
const result=spawnSync(process.execPath,args,{stdio:'inherit',env:environment});
if(result.error)throw result.error;
process.exit(result.status??1);

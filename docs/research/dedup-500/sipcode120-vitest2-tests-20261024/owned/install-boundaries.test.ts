import {it,expect} from 'vitest';
import {runProxy} from '../src/commands/proxy.js';
import {installProxyHook,uninstallProxyHook} from '../src/modules/proxy/install.js';
import {mkdtemp, mkdir, writeFile, readFile} from 'node:fs/promises';
import {join} from 'node:path';
const mixed={permissions:{allow:['Read']},hooks:{PreToolUse:[{matcher:'*',hooks:[{type:'command',command:'node "/owned/sipcode-proxy.mjs"'},{type:'command',command:'echo foreign'}]}]}};
it('reproduces removal of foreign hook in shared group during uninstall',()=>{expect(JSON.stringify(uninstallProxyHook(mixed))).not.toContain('echo foreign');});
it('reproduces removal of foreign hook in shared group during reinstall',()=>{expect(JSON.stringify(installProxyHook(mixed,'/owned/sipcode-proxy.mjs'))).not.toContain('echo foreign');});
it('original command replaces malformed settings on owned filesystem and returns success',async()=>{const home=await mkdtemp(join(process.cwd(),'.dedup-work/sipcode120-vitest2-20261024/owned/install-home-'));await mkdir(join(home,'.claude'));const file=join(home,'.claude/settings.json');const raw='{"permissions":{"allow":["Read"]}, broken';await writeFile(file,raw);const result=await runProxy({install:true},{homeDir:home,stdout:()=>{},stderr:()=>{}});const after=await readFile(file,'utf8');expect(result.exitCode).toBe(0);expect(after).not.toBe(raw);expect(JSON.parse(after).permissions).toBeUndefined();});

import fs from 'node:fs';import vm from 'node:vm';import assert from 'node:assert/strict';
const raw=fs.readFileSync('docs/research/dedup-500/evidence/botnick--telegram-media-downloader/06-dedup.js','utf8');
const rows=[{peer_id:'offline',remote_id:1,file_path:'a',file_size:1,file_hash:'h'},{peer_id:'revoked',remote_id:2}];
const context=vm.createContext({findClusterByHash:()=>rows,getPeer:id=>({peerId:id,name:id,status:id}),recordClusterAudit:()=>{}});
vm.runInContext(raw.replace(/^import .*;\s*$/gm,'').replace(/export /g,''),context);
const hits=vm.runInContext("findHashAcrossCluster('h',1)",context);assert.equal(hits.length,1);assert.equal(hits[0].peerStatus,'offline');
const safe=vm.runInContext("parseClusterRefPath(clusterRefPath('peer/a',42))",context);assert.equal(safe.peerId,'peer/a');assert.equal(safe.remoteId,42);
const unsafe=vm.runInContext("clusterRefPath('p','9007199254740993')",context);assert.equal(unsafe,'_clusterref/p/9007199254740992');
fs.writeFileSync('docs/research/dedup-500/telegram198-controls-20261024/cluster-control-results.json',JSON.stringify({cases:3,scope:'Original full module body in Node VM;imports replaced by DB/peer/audit fixtures, exports stripped. No real DB/network/Telegram.',findings:['Offline status fixture returned despite docstring requiring online;revoked excluded. Actual peer-state vocabulary/caller qualification pending.','Safe identifier42 roundtrips escaped peer id.','String identifier above2^53 rounds through Number;actual remote-id domain not established,not proven production failure.']},null,2)+'\n');console.log('3 cluster wrapper controls passed');

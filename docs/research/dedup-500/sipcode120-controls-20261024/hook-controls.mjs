import {hookReadDedup,realDedupIO} from './hookReadDedup.mts';
import fs from 'node:fs/promises';import assert from 'node:assert/strict';import path from 'node:path';
const root=await fs.mkdtemp(new URL('./hook-owned-',import.meta.url));const file=path.join(root,'source.txt');await fs.writeFile(file,'A'.repeat(800));await fs.utimes(file,1700000000,1700000000);let cache=new Map();let appends=0;
const io={...realDedupIO,loadCache:async()=>new Map(cache),appendEntry:async(p,e)=>{appends++;cache.set(e.filePath,e)},countAssistantTurns:async()=>1,now:()=>new Date('2026-10-24T00:00:00Z')};
const input={tool_name:'Read',tool_input:{file_path:file},session_id:'owned-session',cwd:root};
const first=await hookReadDedup(input,root,io);assert.equal(first.hookOutput,null);assert.equal(appends,1);
// No actual Read tool runs between these pre-tool hooks.
const second=await hookReadDedup(input,root,io);assert.equal(second.hookOutput.hookSpecificOutput.permissionDecision,'deny');
const before=await fs.stat(file);await fs.writeFile(file,'B'.repeat(800));await fs.utimes(file,before.atime,before.mtime);assert.equal((await fs.stat(file)).mtimeMs,before.mtimeMs);assert.equal((await fs.stat(file)).size,before.size);const changed=await hookReadDedup(input,root,io);assert.equal(changed.hookOutput,null);assert.equal(cache.get(file).sha256,(await realDedupIO.hashFile(file)).sha256);
const prior=appends;const partial=await hookReadDedup({...input,tool_input:{file_path:file,offset:0,limit:10}},root,io);assert.equal(partial.hookOutput,null);assert.equal(appends,prior);
await fs.unlink(file);const missing=await hookReadDedup(input,root,io);assert.equal(missing.hookOutput,null);
console.log(JSON.stringify({runtime:process.version,controls_passed:4,findings:['First PreToolUse records observation before actual Read;second identical request denied without any completed Read in fixture','Fresh full-byte hash lets changed same-size content pass after restoredmtime','Partial Read passes and doesnot append cache','Missing source passes rather than denied'],scope:'Original full hook with4 runtime import targets redirected to owned path/cache/prewarm seams and original decision module;real original hashFile reads owned files;warmfill not exercised;no actual tool execution or Claude session',root},null,2));

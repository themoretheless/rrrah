import {it,expect} from 'vitest';
import {parseTranscript} from '../src/modules/transcript/parse.js';
import {analyzeDuplicateReads} from '../src/modules/transcript/analyzers/duplicateReads.js';
function read(input:Record<string,unknown>={file_path:'/owned/a'}){return {type:'tool_use',name:'Read',input};}
function turn(content:unknown[],tokens:number){return JSON.stringify({type:'assistant',message:{content,usage:{input_tokens:tokens}}});}
function parse(raw:string){const r=parseTranscript(raw);if(!r.ok)throw Error('parse refused');return r.value;}
it('different requested slices are still counted as duplicate reads',()=>{const session=parse([turn([read({file_path:'/owned/a',offset:0,limit:10})],50),turn([read({file_path:'/owned/a',offset:100,limit:10})],50)].join('\n'));const d=analyzeDuplicateReads(session);expect(d.distinctFilesRead).toBe(1);expect(d.duplicateReadTokenCost).toBe(50);});
it('repeated calls in one assistant turn multiply its entire input token count',()=>{const session=parse([turn([read()],100),turn([read(),read(),read()],100)].join('\n'));const d=analyzeDuplicateReads(session);expect(d.duplicateReadTokenCost).toBe(300);expect(session.assistantTurns.reduce((n,t)=>n+t.inputTokens,0)).toBe(200);});
it('broad other-entry schema fallback admits negative assistant usage',()=>{const session=parse(turn([read()],-5));expect(session.linesSkipped).toBe(0);expect(session.assistantTurns[0].inputTokens).toBe(-5);});
it('invalid timestamp passes schema and produces non-finite duration',()=>{const raw=JSON.stringify({type:'assistant',timestamp:'not-a-date',message:{content:[],usage:{input_tokens:1}}});const session=parse(raw);expect(session.linesSkipped).toBe(0);expect(Number.isNaN(session.durationSec)).toBe(true);});
it('replaying the same assistant uuid/tool id counts another duplicate read',()=>{const raw=JSON.stringify({type:'assistant',uuid:'same-event',message:{content:[{type:'tool_use',id:'same-tool',name:'Read',input:{file_path:'/owned/a'}}],usage:{input_tokens:100}}});const session=parse(raw+'\n'+raw);expect(session.assistantTurns.length).toBe(2);expect(analyzeDuplicateReads(session).duplicateReadTokenCost).toBe(100);});

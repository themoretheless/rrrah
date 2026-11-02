import {it,expect} from 'vitest';
import {createRequire} from 'node:module';
import {hookAstRead} from '../src/modules/proxy/hookAstRead.js';
import {extractTsSymbols} from '../src/modules/proxy/ast/ts-symbols.js';
const require=createRequire(import.meta.url);const Parser=require('tree-sitter');const TS=require('tree-sitter-typescript');
const good='export function target() { return 1; }\n'+'// padding\n'.repeat(220);
async function run(content:string){return hookAstRead({session_id:'owned',transcript_path:'',cwd:'/owned',permission_mode:'default',hook_event_name:'PreToolUse',tool_name:'Read',tool_input:{file_path:'/owned/source.ts'}},'/owned',{readFile:async()=>content,read:async()=>JSON.stringify({tool:'Grep',kind:'grep-pattern',pattern:'target',capturedAtMs:0})+'\n',append:async()=>{},now:()=>new Date('2026-10-24T00:00:00Z')});}
it('native parse tree has error but original extractor and hook still trim',async()=>{const content=good+'function broken( {\n';const parser=new Parser();parser.setLanguage(TS.typescript);expect(parser.parse(content).rootNode.hasError).toBe(true);expect(extractTsSymbols('source.ts',content).some(s=>s.name==='target')).toBe(true);const result=await run(content);expect(result.hookOutput?.hookSpecificOutput.updatedInput).toBeDefined();});
it('native valid source positive control extracts and trims',async()=>{const parser=new Parser();parser.setLanguage(TS.typescript);expect(parser.parse(good).rootNode.hasError).toBe(false);const result=await run(good);expect(result.hookOutput?.hookSpecificOutput.updatedInput).toBeDefined();});

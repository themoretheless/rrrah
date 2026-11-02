import {it,expect} from 'vitest';
import {scoreSymbols,pickRelevantSymbols} from '../src/modules/proxy/ast/relevance.js';
const symbol={name:'calculateInvoice',kind:'function' as const,startLine:10,endLine:15,isTopLevel:true as const,isExported:true};
it('old and fresh search signals have equal score and eligibility',()=>{const signal={tool:'Grep',kind:'grep-pattern' as const,pattern:'calculateInvoice',capturedAtMs:0};const old=scoreSymbols([symbol],[signal]);const fresh=scoreSymbols([symbol],[{...signal,capturedAtMs:Date.parse('2026-10-24T00:00:00Z')}]);expect(old).toEqual(fresh);expect(pickRelevantSymbols(old)).toHaveLength(1);});
it('a single common character crosses the default trim threshold',()=>{const scored=scoreSymbols([symbol],[{tool:'Grep',kind:'grep-pattern',pattern:'a',capturedAtMs:0}]);expect(scored[0].confidence).toBeGreaterThan(0.7);expect(pickRelevantSymbols(scored)).toHaveLength(1);});

import {expect,test} from 'vitest';
import {csv,costs,total,type UsageEvent} from './data';
const event:UsageEvent={id:'event',session:'session',occurredAt:1,source:'api',provider:'=HYPERLINK("bad")',model:'model',input:null,cacheRead:0,cacheWrite:null,output:12,credits:null,cost:'0.000000000001',currency:'USD',kind:'回合汇总'};
test('CSV escapes formulas and preserves unknown metadata',()=>{const output=csv([event]);expect(output).toContain("'=HYPERLINK");expect(output).toContain('未知');expect(output).not.toContain('apiKey');});
test('usage null is never zero, and currencies remain separate with exact sums',()=>{expect(total([event],'input')).toBe('未知');expect(total([event],'cacheRead')).toBe('0');expect(costs([event,event,{...event,currency:'CNY',cost:'1'}])).toBe('USD 0.000000000002 · CNY 1');});

import { dhash,compare } from './mod.ts';
const rows=JSON.parse(await Deno.readTextFile('../dhash440-orientation-20261021/cases.json'));
for(const row of rows){row.tagged_hash=await dhash(row.tagged);row.expected_hash=await dhash(row.physical);row.distance=compare(row.tagged_hash,row.expected_hash);row.matches=row.distance===0;}
await Deno.writeTextFile('../../docs/research/dedup-500/dhash440-orientation-controls-20261021/result.json',JSON.stringify({cases:rows,scope:'Original dhash/Sharp pipeline; expected pixels authored independently in Python by eight coordinate mappings, not Sharp flip/rotate calls. Compares hashes, not internal decoded pixel buffer.'},null,2)+'\n');
console.log(JSON.stringify(rows.map((r:any)=>({orientation:r.orientation,distance:r.distance,matches:r.matches}))));

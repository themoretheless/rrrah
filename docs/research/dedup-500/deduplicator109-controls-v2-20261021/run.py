"""Authored controls against original JSON.lua using Lua 5.1, no Lightroom."""
from pathlib import Path
import hashlib,json
from lupa.lua51 import LuaRuntime
R=Path(__file__).resolve().parents[4]
p=R/'.dedup-work/deduplicator109-20261021/deduplicator.lrdevplugin/JSON.lua'
b=p.read_bytes();lua=LuaRuntime(unpack_returned_tuples=True);lua.globals().JSON=lua.execute(b.decode())
checks=[
 ('strict empty containers preserve object/array identity',r'''JSON.strictTypes=true; assert(JSON:encode(JSON:decode('{}'))=='{}'); assert(JSON:encode(JSON:decode('[]'))=='[]')'''),
 ('unicode surrogate and escaped path roundtrip',r'''local s='{"duplicates":[["/synthetic/\\uD83D\\uDE00.jpg","/synthetic/a\\\\b.jpg"]]}'; local t=JSON:decode(s); assert(t.duplicates[1][1]=='/synthetic/'..string.char(240,159,152,128)..'.jpg'); assert(JSON:encode(JSON:decode(JSON:encode(t)))==JSON:encode(t))'''),
 ('trailing garbage rejected',r'''assert(not pcall(function()JSON:decode('[]xyz')end))'''),
 ('unknown escape accepted as literal character',r'''assert(JSON:decode('"a\\q"')=='aq')'''),
 ('duplicate object keys retain last value',r'''assert(JSON:decode('{"duplicates":[],"duplicates":[["x"]]}').duplicates[1][1]=='x')'''),
 ('number-object unary minus returns positive value',r'''assert(-JSON:asNumber('3')==3)'''),
]
for name,code in checks:
 lua.execute(code);print('PASS',name)
print('6 controls passed; Lua runtime:',lua.eval('_VERSION'))
print('JSON source SHA256:',hashlib.sha256(b).hexdigest())
print('Malformed JSON and number-object cases are not claimed normal imgsum output failures')

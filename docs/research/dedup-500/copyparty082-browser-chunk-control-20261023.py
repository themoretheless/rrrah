#!/usr/bin/env python3
"""Compare original browser/server chunk-size functions in the JS safe-integer domain."""
from pathlib import Path
import hashlib
import json
import subprocess
import sys

checkout, output = [Path(p).resolve() for p in sys.argv[1:]]
sys.path.insert(0, str(checkout))
from copyparty.up2k import up2k_chunksize

root = Path(__file__).parent
tree = json.loads((root / "evidence/9001--copyparty/tree.json").read_text())
pins = {x["path"]: x["sha"] for x in tree["tree"] if x["type"] == "blob"}
source = checkout / "copyparty/web/up2k.js"
for rel in ["copyparty/up2k.py", "copyparty/web/up2k.js"]:
    b = (checkout / rel).read_bytes()
    assert hashlib.sha1(b"blob " + str(len(b)).encode() + b"\0" + b).hexdigest() == pins[rel]
maximum = 2**53 - 1
values = {0, 1, maximum}
for power in range(54):
    values.update(v for v in [2**power-1, 2**power, 2**power+1] if 0 <= v <= maximum)
for size in list(values):
    chunk = up2k_chunksize(size)
    for count in [256, 4096]:
        values.update(v for v in [chunk*count-1, chunk*count, chunk*count+1] if 0 <= v <= maximum)
values = sorted(values)
javascript = r'''
const fs = require('node:fs'), vm = require('node:vm'), crypto = require('node:crypto');
const source = fs.readFileSync(process.argv[1], 'utf8');
const body = source.match(/function get_chunksize\(filesize\) \{[\s\S]*?\n    \}/)[0];
const fn = vm.runInNewContext('(' + body + ')');
const sizes = JSON.parse(process.argv[2]);
if (!sizes.every(x => Number.isSafeInteger(x) && x >= 0)) throw Error('input domain');
console.log(JSON.stringify({bodySha256:crypto.createHash('sha256').update(body).digest('hex'),
    rows:sizes.map(size => ({size, chunk:fn(size)}))}));
'''
run = subprocess.run(["node", "-e", javascript, str(source), json.dumps(values)],
                     capture_output=True, text=True, check=True, timeout=30)
browser = json.loads(run.stdout)
rows = []
for row in browser["rows"]:
    size, js = row["size"], row["chunk"]
    py = up2k_chunksize(size)
    count = (size + py - 1) // py
    assert js == py
    assert count <= 256 or py >= 32*1024**2 and count <= 4096
    rows.append({"bytes": size, "browser_chunk": js, "server_chunk": py, "chunks": count})
result = {"cases": len(rows), "matched": len(rows), "rows": rows,
          "original_browser_function_sha256": browser["bodySha256"],
          "browser_source_sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
          "node": subprocess.check_output(["node", "--version"], text=True).strip(),
          "scope": "Complete original pure get_chunksize and up2k_chunksize bodies, Node/Python. Only extracted JS function evaluated, no browser module, DOM, network, worker, hashing or file-size IO. Finite safe-integer boundaries, not exhaustive JS domain or performance proof."}
assert not output.exists()
output.write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps({"cases": len(rows), "matched": len(rows)}))

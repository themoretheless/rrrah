#!/usr/bin/env python3
import hashlib,json
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 old=D/'dedup-zero-smoothing-borrow-v4-raster-verification-20261012.json';m=json.loads(old.read_text());assert all(sha(k)==v for k,v in m['pins'].items());assert m['passed']==48 and m['ignored']==3 and len(m['suites'])==2
 m['scope']=m['scope'].replace('Five ignored manual controls','Three ignored manual controls');m['pins'].update({str(old):sha(old),str(Path(__file__)):sha(__file__)});m['correction']='Original report numeric counts48passed/3ignored were correct; copied prose fiveignored corrected tothree. No source/test results changed.';out=D/'dedup-zero-smoothing-borrow-v4-raster-verification-v2-20261012.json';assert not out.exists();out.write_text(json.dumps(m,indent=2)+'\n');print('Verified raster gate48passes/3ignored; corrected prose only')
if __name__=='__main__':main()

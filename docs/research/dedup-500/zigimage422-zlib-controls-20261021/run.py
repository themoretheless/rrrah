import subprocess,zlib,json,hashlib
from pathlib import Path
folder=Path(__file__).resolve().parent;root=Path('.dedup-work/zigimage422-20261021');binary=root/'zlib-control';subprocess.run(['clang','-std=c99','-O2','-I',str(root/'include'),str(folder/'control.c'),'-lm','-o',str(binary)],check=True)
payload=bytes(range(256))*16;rows=[]
for level in [0,1,9]:
 data=zlib.compress(payload,level)
 for kind,stream in [('valid',data),('bad_adler',data[:-1]+bytes([data[-1]^1])),('missing_adler',data[:-4])]:
  f=root/'zlib-control.bin';f.write_bytes(stream);p=subprocess.run([str(binary),str(f)],capture_output=True,timeout=5)
  try:ref=zlib.decompress(stream);reference='accepted';assert ref==payload
  except zlib.error:reference='rejected'
  assert p.returncode==0 and p.stdout==payload
  rows.append(dict(level=level,variant=kind,reference_zlib=reference,stb_returncode=p.returncode,output_matches_payload=True,stream_sha256=hashlib.sha256(stream).hexdigest()))
out=dict(cases=rows,scope='Original C fixed-buffer zlib API with8192byteoutputcap; 4096byteauthoredpayload; Pythonzlibindependentdecoder comparison. ChecksumhandlingnotwholePNGCRC/JPEG/integrityqualification; no arbitrary amplification input.')
(folder/'result.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2))

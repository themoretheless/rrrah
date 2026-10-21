import pathlib,subprocess,tempfile,json,hashlib,struct,ast
src=pathlib.Path('.dedup-work/media459-20261021').resolve();out=pathlib.Path('.dedup-work/media459-controls-20261021').resolve();out.mkdir(exist_ok=True)
p=subprocess.run(['c++','-std=c++17','-O1',str(src/'main_cli.cpp'),'-o',str(out/'main_cli')],capture_output=True,text=True);assert p.returncode==0,p.stderr
cases={}
def run(root,similar=False):
 p=subprocess.run([str(out/'main_cli'),str(root)]+(['--similar'] if similar else []),capture_output=True,text=True,timeout=30);assert p.returncode==0,p.stderr;return p.stdout
with tempfile.TemporaryDirectory(prefix='rrrah-media459-') as tmp:
 root=pathlib.Path(tmp);exact=root/'exact';exact.mkdir();(exact/'a.txt').write_bytes(b'Aa');(exact/'b.txt').write_bytes(b'BB');v=run(exact);assert 'EXACT|1.0' in v;cases['distinct_bytes_reported_exact']={'inputs_hex':['4161','4242'],'same_size':True,'stdout':v}
 audio=root/'audio';audio.mkdir();(audio/'aaaaab.mp3').write_bytes(b'A');(audio/'aaaaac.mp3').write_bytes(b'B');v=run(audio,True);assert 'SIMILAR|2.58' in v;cases['audio_filename_score_exceeds_one']={'stdout':v,'audio_decoder_used':False}
 images=root/'images';images.mkdir()
 body=bytes(c for y in range(8) for x in range(8) for c in [x*30]*3)
 for marker in [0,1]:
  header=bytearray(54);header[:2]=b'BM';struct.pack_into('<I',header,2,54+len(body));struct.pack_into('<I',header,10,54);struct.pack_into('<I',header,14,40);struct.pack_into('<ii',header,18,8,8);struct.pack_into('<HH',header,26,1,24);header[6]=marker;(images/f'{marker}.bmp').write_bytes(header+body)
 v=run(images,True);assert 'EXACT|' not in v and 'SIMILAR|' not in v;cases['same_pixels_gradient_rejected']={'stdout':v,'recipe':'Two8x8 ascending grayscale ramps, identical pixel bytes, only BMPreserved headerbyte differs. Differencehash0 rejected before distance comparison; no decoder error injected.'}
script=src/'office_comparer_batch.py';tree=ast.parse(script.read_bytes());ns={};exec(compile(ast.Module(body=[n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name in {'compare_sheets_fast','compare_excel_fast'}],type_ignores=[]),str(script),'exec'),ns)
score=ns['compare_excel_fast']({'same':[[1]]},{'same':[[1],[999]],'extra':[[123]]});assert score==1;cases['excel_ignores_extra_rows_and_sheets']={'score':score,'method':'Two original Python function nodes unchanged; synthetic in-memory row arrays, no fake workbook parser.'}
r={'method':'Whole original main_cli.cpp and originalstb_image.h compiled unchanged with AppleC++17 on macOSarm64, nonWindows hash path. Actual CLI process on owned tempfile files only, no GUI, deletions, office subprocess or packaged executables.','source_sha256':{name:hashlib.sha256((src/name).read_bytes()).hexdigest() for name in ['main_cli.cpp','stb_image.h','office_comparer_batch.py']},'controls':cases,'limits':['NonWindows polynomial hash collision does not qualify WindowsCryptoAPIbranch.','stb header compiled but not wholly reviewed; source/package provenance and all decoder cases remain open.','No claim that arbitrary MP3bytes are valid audio; filename-only classification demonstrated.']};pathlib.Path('docs/research/dedup-500/media459-controls-20261021/result.json').write_text(json.dumps(r,indent=2)+'\n');print(json.dumps(r,indent=2))

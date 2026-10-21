from pathlib import Path
import importlib.util,subprocess,sys,tempfile,json
root=Path.cwd();original=root/'.dedup-work/visualvault076-20261021/scripts/convert_benchmark_output.py'
spec=importlib.util.spec_from_file_location('original_converter',original);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
with tempfile.TemporaryDirectory(prefix='visualvault076-parser-') as name:
 t=Path(name);src=t/'empty.txt';out=t/'converted.txt';src.write_text('');r=subprocess.run([sys.executable,str(original),str(src),str(out)],capture_output=True,text=True)
 assert r.returncode==0;assert out.read_text()=='test dummy_benchmark ... bench: 1000 ns/iter (+/- 0)\n'
 print('PASS empty input exits0 and writes synthetic1000ns dummy record');print(r.stderr.strip())
 mixed='test one ... bench: 100 ns/iter (+/- 5)\ntwo time: [1 ns 2 ns 3 ns]\n';parsed=m.parse_benchmark_output(mixed);assert parsed==[{'name':'one','value':100,'variance':5}]
 assert m.parse_criterion_output(mixed)==[{'name':'two','value':2,'variance':1}]
 print('PASS mixed input drops otherwise parseable Criterion record after Bencher match')
 parsed=m.parse_benchmark_output('missing: {"unit":"ns"}\n');assert parsed==[{'name':'missing','value':0,'variance':0}]
 print('PASS JSON without estimate produces0ns timing record');print(json.dumps({'authored_controls_passed':3,'original_benchmarks_run':0}))

import importlib.util,json,subprocess,sys
from pathlib import Path
base=Path(__file__).resolve().parent
p=base/'intra095-20261024/deduplicator/rwlock.py'
spec=importlib.util.spec_from_file_location('original_rwlock',p);mod=importlib.util.module_from_spec(spec);spec.loader.exec_module(mod)
if len(sys.argv)>1:
 lock=mod.RWLock();lock.writer_acquire();lock.reader_acquire(blocking=False);print('reader returned',flush=True);sys.exit(0)
lock=mod.RWLock()
try:lock.reader_release()
except RuntimeError as e:overrelease=str(e)
switch=lock._RWLock__read_switch
mutex_usable=switch._LightSwitch__mutex.acquire(blocking=False)
if mutex_usable:switch._LightSwitch__mutex.release()
other=mod.RWLock();other.writer_acquire()
try:other.writer_acquire(blocking=False)
except RuntimeError as e:nonblocking=str(e)
other.writer_release()
gate_usable=other._RWLock__no_readers.acquire(blocking=False)
if gate_usable:other._RWLock__no_readers.release()
try:
 child=subprocess.run([sys.executable,str(Path(__file__).resolve()),'reader'],capture_output=True,text=True,timeout=1)
 blocked=False
except subprocess.TimeoutExpired:
 blocked=True
assert not mutex_usable and not gate_usable and blocked
print(json.dumps(dict(overrelease_error=overrelease,read_switch_mutex_available_after_error=mutex_usable,failed_reentrant_writer_error=nonblocking,readers_gate_available_after_original_writer_release=gate_usable,nonblocking_reader_blocked_over_one_second=blocked,scope='Unchanged original RWLock module;two owned lock objects plus bounded child;private lock availability probes;not original Cython wrapper tests'),sort_keys=True))

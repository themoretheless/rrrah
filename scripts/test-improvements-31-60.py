"""Real process interruption/reader and watchdog regressions for RR-0034/0055."""
import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import threading
import time
import unittest
from unittest.mock import patch

SCRIPTS = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPTS))
from qualification_command import run_logged
spec = importlib.util.spec_from_file_location('bench', SCRIPTS/'bench-report.py')
bench = importlib.util.module_from_spec(spec); spec.loader.exec_module(bench)

class QualificationTests(unittest.TestCase):
    def test_normal_and_failure_preserve_diagnostics(self):
        logs=[]
        self.assertIn('done',run_logged([sys.executable,'-c','print("done")'],logs,5))
        with self.assertRaises(RuntimeError):run_logged([sys.executable,'-c','import sys; print("bad",file=sys.stderr); sys.exit(7)'],logs,5)
        self.assertEqual(logs[-1]['exit_code'],7);self.assertIn('bad',logs[-1]['output']);self.assertFalse(logs[-1]['timed_out'])

    @unittest.skipUnless(os.name=='posix','real POSIX process tree')
    def test_timeout_kills_resistant_child_and_never_succeeds(self):
        with tempfile.TemporaryDirectory()as folder:
            pid=Path(folder)/'child.pid'
            child=f'import signal,os,time; signal.signal(signal.SIGTERM,signal.SIG_IGN); open({str(pid)!r},"w").write(str(os.getpid())); time.sleep(60)'
            parent=f'import subprocess,sys,time; subprocess.Popen([sys.executable,"-c",{child!r}]); print("parent",flush=True); time.sleep(60)'
            logs=[];start=time.monotonic()
            with self.assertRaises(TimeoutError):run_logged([sys.executable,'-c',parent],logs,1)
            self.assertLess(time.monotonic()-start,6);self.assertTrue(logs[0]['timed_out']);self.assertIn('parent',logs[0]['output'])
            state=subprocess.run(['ps','-p',pid.read_text(),'-o','stat='],capture_output=True,text=True,timeout=2)
            self.assertIn(state.returncode,(0,1));self.assertTrue(not state.stdout.strip() or state.stdout.strip().startswith('Z'),state.stdout)

    def test_x3f_deadline_rejects_zero_negative_and_nonfinite_before_source_io(self):
        for value in ['0','-1','nan','inf']:
            result=subprocess.run([sys.executable,str(SCRIPTS/'qualify-x3f-sd10.py'),'missing',
                '--libraw-root','missing','--target-dir','missing','--report','unused','--timeout',value],capture_output=True,text=True,timeout=3)
            self.assertEqual(result.returncode,2);self.assertIn('finite and positive',result.stderr)

    def test_atomic_failure_preserves_old_json_and_mode(self):
        with tempfile.TemporaryDirectory()as folder:
            path=Path(folder)/'report.json';path.write_text('{"old":true}');path.chmod(0o640)
            with patch.object(bench.os,'replace',side_effect=OSError('injected replace failure')):
                with self.assertRaises(OSError):bench.atomic_write_report(path,'{"new":true}')
            self.assertEqual(json.loads(path.read_text()),{'old':True});self.assertEqual(list(path.parent.glob('*.tmp')),[])
            bench.atomic_write_report(path,'{"new":true}');self.assertEqual(path.stat().st_mode&0o777,0o640)

    @unittest.skipUnless(os.name=='posix','POSIX interruption and orphan cleanup')
    def test_kill_at_publication_old_or_new_complete_then_cleanup(self):
        with tempfile.TemporaryDirectory()as folder:
            path=Path(folder)/'report.json';path.write_text('{"version":"old"}')
            for after in [False,True]:
                code=f'''import importlib.util,sys,time
spec=importlib.util.spec_from_file_location('bench',{str(SCRIPTS/'bench-report.py')!r});b=importlib.util.module_from_spec(spec);spec.loader.exec_module(b)
original=b.os.replace
def stop(*args):
 if {after!r}:original(*args)
 print('ready',flush=True)
 time.sleep(60)
b.os.replace=stop
b.atomic_write_report(b.Path({str(path)!r}),'{{"version":"new"}}')
'''
                process=subprocess.Popen([sys.executable,'-c',code],stdout=subprocess.PIPE,text=True)
                try:
                    # Bounded readiness; never leave a blocked readline on a broken child.
                    import selectors
                    with selectors.DefaultSelector()as selector:
                        selector.register(process.stdout,selectors.EVENT_READ);self.assertTrue(selector.select(5));self.assertEqual(process.stdout.readline().strip(),'ready')
                    process.kill();process.wait(timeout=5)
                    self.assertEqual(json.loads(path.read_text())['version'],'new' if after else 'old')
                    bench.atomic_write_report(path,'{"version":"old"}')
                    self.assertEqual(list(path.parent.glob('*.tmp')),[])
                finally:
                    if process.poll()is None:process.kill();process.wait(timeout=5)
                    process.stdout.close()

    @unittest.skipUnless(os.name=='posix','POSIX file lock')
    def test_busy_writer_has_bounded_wait_and_does_not_remove_its_temp(self):
        import fcntl
        with tempfile.TemporaryDirectory()as folder:
            path=Path(folder)/'report.json';path.write_text('{"old":true}')
            lock=path.parent/f'.{path.name}.rrrah-report.lock'
            with lock.open('a+b')as guard:
                fcntl.flock(guard.fileno(),fcntl.LOCK_EX)
                temporary=path.parent/f'.{path.name}.rrrah-report-{"a"*32}.tmp';temporary.write_text('active')
                with patch.object(bench,'REPORT_LOCK_TIMEOUT_SECONDS',0.05):
                    start=time.monotonic()
                    with self.assertRaises(TimeoutError):bench.atomic_write_report(path,'{}')
                    self.assertLess(time.monotonic()-start,0.5)
                self.assertEqual(temporary.read_text(),'active');self.assertEqual(json.loads(path.read_text()),{'old':True})
            bench.atomic_write_report(path,'{"new":true}');self.assertFalse(temporary.exists())

    @unittest.skipUnless(os.name=='posix','POSIX orphan recovery')
    def test_literal_filename_orphan_recovery(self):
        with tempfile.TemporaryDirectory()as folder:
            path=Path(folder)/'report[1]?.json'
            orphan=path.parent/f'.{path.name}.rrrah-report-{"b"*32}.tmp'
            orphan.write_text('interrupted')
            unrelated=path.parent/'.unrelated.rrrah-report-cccc.tmp'
            unrelated.write_text('keep')
            bench.atomic_write_report(path,'{"ok":true}')
            self.assertFalse(orphan.exists());self.assertEqual(unrelated.read_text(),'keep')
            self.assertTrue(json.loads(path.read_text())['ok'])

    def test_concurrent_reader_never_observes_partial_document(self):
        with tempfile.TemporaryDirectory()as folder:
            path=Path(folder)/'report.json';bench.atomic_write_report(path,json.dumps({'n':0,'body':'x'*10000}))
            stop=threading.Event();errors=[];reads=[]
            def reader():
                while not stop.is_set():
                    try:
                        row=json.loads(path.read_text());assert row['body']=='x'*10000;reads.append(row['n'])
                    except Exception as error:errors.append(repr(error));break
            thread=threading.Thread(target=reader);thread.start()
            try:
                for n in range(1,50):bench.atomic_write_report(path,json.dumps({'n':n,'body':'x'*10000}))
            finally:stop.set();thread.join(5)
            self.assertTrue(reads);self.assertFalse(errors,errors);self.assertFalse(thread.is_alive())

if __name__=='__main__':unittest.main()

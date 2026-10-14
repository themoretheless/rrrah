#!/usr/bin/env python3
"""Wait for the already-owned managed gate, audit it, then benchmark once."""
from pathlib import Path
import hashlib,json,os,subprocess,time,traceback
R=Path(__file__).resolve().parents[1];D=R/'docs/research';gate=D/'dedup-gradient-masked-lanes-managed-20261014.json';O=D/'dedup-gradient-masked-lanes-qualification-queue-v2-20261014.json';assert not O.exists()
state=dict(status='waiting_for_existing_managed_gate',producer_pid=os.getpid(),gate=str(gate),producer_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),steps=[],scope='Does not start or restart the existing gate. After terminal pass, independently audit the complete raster inventory, then run one actual public managed API comparison. Production checkout unchanged.')
def save():
    tmp=O.with_suffix('.tmp');tmp.write_text(json.dumps(state,indent=2)+'\n');tmp.replace(O)
save()
try:
    while True:
        d=json.loads(gate.read_text())
        if d['status']=='passed_pending_audit':break
        assert d['status'] in ['building','listing','testing'],d['status']
        # Read-only liveness observation; never restart based on a state file.
        try:
            os.kill(d['producer_pid'],0)
        except PermissionError:
            state['liveness_observation']='Sandbox denies signal0; owner must poll original managed tool session70707. This is not a terminal observation and never triggers restart.'
            save()
        time.sleep(5)
    for stage,name in [('auditing','audit-dedup-gradient-masked-lanes-managed-20261014.py'),('benchmarking','benchmark-dedup-gradient-masked-lanes-managed-20261014.py')]:
        script=R/'scripts'/name;log=Path('/private/tmp')/(name+'.queue.log')
        with log.open('x') as stdout:
            env=os.environ.copy();env['PYTHONDONTWRITEBYTECODE']='1'
            child=subprocess.Popen(['python3',str(script)],cwd=R,env=env,stdout=stdout,stderr=subprocess.STDOUT)
            state.update(status=stage,child_pid=child.pid);save();code=child.wait()
        state['steps'].append(dict(script=str(script),script_sha256=hashlib.sha256(script.read_bytes()).hexdigest(),returncode=code,log=str(log)));save();assert code==0,(stage,code)
    state['status']='completed_public_benchmark_pending_independent_audit';save()
except BaseException:
    state.update(status='failed_without_restart',error=traceback.format_exc());save();raise

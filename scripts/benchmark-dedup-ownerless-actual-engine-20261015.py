#!/usr/bin/env python3
import hashlib
import json
import os
from pathlib import Path
import re
import resource
import statistics
import subprocess

ROOT = Path(__file__).resolve().parents[1]
DIRECTORY = ROOT / 'docs/research'
AUDIT = DIRECTORY / 'dedup-ownerless-actual-engine-audit-20261015.json'
audit = json.loads(AUDIT.read_text())
assert audit['failed'] == 0 and audit['passed'] == 25
WORK = Path('/private/tmp/rrrah-ownerless-actual-engine-benchmark-20261015')
WORK.mkdir(exist_ok=False)
OUTPUT = DIRECTORY / 'dedup-ownerless-actual-engine-benchmark-20261015.json'
assert not OUTPUT.exists()
sha = lambda path: hashlib.sha256(Path(path).read_bytes()).hexdigest()
exe = audit['executable']
assert sha(exe) == audit['executable_sha256']
cases = [('empty2', 2, 0, 1000, 'equal'), ('empty8', 8, 0, 200, 'equal'), ('64k8', 8, 65536, 50, 'equal'), ('64k24', 24, 65536, 5, 'equal'), ('2m8', 8, 2 * 1024 * 1024, 3, 'equal'), ('early8', 8, 65536, 50, 'early'), ('late8', 8, 65536, 50, 'late'), ('sizes8', 8, 65536, 50, 'sizes'), ('mixed8', 8, 65536, 50, 'mixed')]
report = dict(status='running', producer_pid=os.getpid(), producer_sha256=sha(__file__), correctness_audit_sha256=sha(AUDIT), executable=exe, executable_sha256=sha(exe), cases=cases, owner_kinds=['none', 'all', 'mixed'], rounds=7, rows=[], scope='Actual original guarded collection engine plus original exact/source/dependency/scheduler code; same binary selects original owner scans versus presence-selected slice once before callbacks. 9datafamilies x3ownerkinds x7alternating rounds x2freshprocesses. Synthetic provider oracle, real file IO, report pairs, source/model guards, callbacks, generations and managed credits. Warm filesystem expected; child CPU includes setup/cleanup. Concurrent full compilation/native campaign prevents isolated performance/significance claim. Not full library/default/release/LTO/real image provider/decode quality/colddevice/RSS/platform qualification or production adoption.')

def save():
    temporary = OUTPUT.with_suffix('.tmp')
    temporary.write_text(json.dumps(report, indent=2) + '\n')
    temporary.replace(OUTPUT)

save()
for label, count, size, iterations, mode in cases:
    pairs = count * (count - 1) // 2
    matches = pairs if mode == 'equal' else sum(mode == 'mixed' and i % 3 == j % 3 for i in range(count) for j in range(i + 1, count))
    for owners in report['owner_kinds']:
        for round_index in range(report['rounds']):
            variants = ['baseline', 'candidate'] if round_index % 2 == 0 else ['candidate', 'baseline']
            paired = []
            for variant in variants:
                assert sha(exe) == report['executable_sha256']
                environment = os.environ.copy()
                environment.update(DEDUP_BENCH_FILES=str(count), DEDUP_BENCH_BYTES=str(size), DEDUP_BENCH_ITERATIONS=str(iterations), DEDUP_BENCH_CONTENT=mode, DEDUP_BENCH_OWNERS=owners, DEDUP_BENCH_VARIANT=variant)
                command = [exe, '--ignored', '--exact', audit['benchmark'], '--nocapture', '--test-threads', '1']
                log = WORK / f'{label}-{owners}-{round_index}-{variant}.log'
                cpu_before = resource.getrusage(resource.RUSAGE_CHILDREN)
                with log.open('x') as stream:
                    child = subprocess.Popen(command, cwd=ROOT, env=environment, stdout=stream, stderr=subprocess.STDOUT)
                    report['active'] = dict(case=label, owners=owners, round=round_index, variant=variant, child_pid=child.pid)
                    save()
                    code = child.wait()
                cpu_after = resource.getrusage(resource.RUSAGE_CHILDREN)
                cpu_seconds = cpu_after.ru_utime + cpu_after.ru_stime - cpu_before.ru_utime - cpu_before.ru_stime
                text = log.read_text()
                found = re.search(r'DEDUP_ENGINE_NS=(\d+) ACCEPTED=(\d+) VISUAL_CALLS=(\d+) CHECKPOINTS=(\d+)', text)
                if code or not found:
                    report.update(status='failed_child', failed_log=str(log), failed_returncode=code)
                    save()
                    raise SystemExit(code or 1)
                elapsed, accepted, visual, checkpoints = map(int, found.groups())
                assert accepted == iterations * pairs and visual == iterations * (pairs - matches)
                assert '1 passed; 0 failed; 0 ignored; 0 measured;' in text
                row = dict(case=label, owners=owners, round=round_index, variant=variant, nanoseconds=elapsed, cpu_seconds=cpu_seconds, accepted=accepted, visual_calls=visual, checkpoints=checkpoints, log=str(log), log_sha256=sha(log))
                paired.append(row)
                report['rows'].append(row)
                save()
            assert paired[0]['checkpoints'] == paired[1]['checkpoints']
results = []
for label, *_ in cases:
    for owners in report['owner_kinds']:
        wall = []
        cpu = []
        for round_index in range(report['rounds']):
            pair = {row['variant']: row for row in report['rows'] if row['case'] == label and row['owners'] == owners and row['round'] == round_index}
            wall.append(pair['candidate']['nanoseconds'] / pair['baseline']['nanoseconds'])
            cpu.append(pair['candidate']['cpu_seconds'] / pair['baseline']['cpu_seconds'])
        results.append(dict(case=label, owners=owners, paired_wall_ratios=wall, paired_cpu_ratios=cpu, median_wall_ratio=statistics.median(wall), median_cpu_ratio=statistics.median(cpu)))
assert len(report['rows']) == 378 and sha(exe) == report['executable_sha256'] and sha(AUDIT) == report['correctness_audit_sha256']
report.update(status='completed_pending_independent_audit', active=None, results=results)
save()
print('378actual engine processes complete; independent audit and adoption decision pending')

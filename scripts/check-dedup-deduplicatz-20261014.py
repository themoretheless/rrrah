#!/usr/bin/env python3
"""Original hashset/queue/worker controls; io_uring and real xxHash not executed."""
from pathlib import Path
import hashlib
import json
import subprocess

R = Path(__file__).resolve().parents[1]
E = R / 'docs/research/dedup-500/evidence/x-stp--deduplicatz'
F = R / 'docs/research/dedup-deduplicatz-controls-20261014'
F.mkdir(exist_ok=False)
tree = json.loads((E / 'tree.json').read_text())
names = {'LICENSE': '01-LICENSE', 'README.md': '00-README.md', 'deduplicatz.c': '02-deduplicatz.c'}
for item in tree['tree']:
    data = (E / names[item['path']]).read_bytes()
    assert hashlib.sha1(b'blob ' + str(len(data)).encode() + b'\0' + data).hexdigest() == item['sha']
text = (E / '02-deduplicatz.c').read_text()
body = text[text.index('#define CHUNK_SIZE'):text.index('// --- progress ---')]
header = '''#include <pthread.h>
#include <unistd.h>
#include <sys/uio.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <stdint.h>
#include <stdatomic.h>
#include <assert.h>
/* Controlled byte hash for worker framing tests only, not real xxHash. */
static uint64_t XXH3_64bits(const void* p, size_t n) {
    const unsigned char* bytes=p; uint64_t h=1469598103934665603ULL;
    for(size_t i=0;i<n;i++) h=(h^bytes[i])*1099511628211ULL;
    return h;
}
'''
driver = r'''
static void push(char* bytes,size_t length) {
    chunk_t* chunk=malloc(sizeof(*chunk));
    *chunk=(chunk_t){bytes,length,1}; queue_push(&queue,chunk);
}
int main(int argc,char** argv) {
    assert(argc==2);
    if(!strcmp(argv[1],"zero")) {
        hashset_init(&hset,101);
        assert(hashset_insert(&hset,0)==1);
        assert(hashset_insert(&hset,0)==1);
        assert(atomic_load(&hset.count)==2);
    } else if(!strcmp(argv[1],"probe")) {
        hashset_init(&hset,101);
        for(uint64_t i=0;i<64;i++) assert(hashset_insert(&hset,1+i*101)==1);
        assert(hashset_insert(&hset,1+64*101)==0);
        assert(atomic_load(&hset.count)==64);
    } else if(!strcmp(argv[1],"rehash")) {
        hashset_init(&hset,101);
        for(size_t i=0;i<65;i++) hset.table[i]=1+i*211;
        atomic_store(&hset.count,65); hashset_rehash(&hset);
        assert(hset.size==211);
        size_t occupied=0;for(size_t i=0;i<hset.size;i++) occupied+=hset.table[i]!=0;
        assert(occupied==64 && atomic_load(&hset.count)==65);
    } else {
        hashset_init(&hset,101);queue_init(&queue);
        FILE* output=tmpfile();assert(output);out_fd=fileno(output);
        char a[]="alpha\n",b[]="alpha\n";
        if(!strcmp(argv[1],"duplicate")){push(a,6);push(b,6);}
        else if(!strcmp(argv[1],"eof")){push(a,5);}
        else if(!strcmp(argv[1],"split")){push(a,2);push(a+2,4);}
        else if(!strcmp(argv[1],"borrow")){push(a,6);memcpy(a,"omega\n",6);}
        else if(!strcmp(argv[1],"long")) {
            char* longline=malloc(MAX_LINE_SIZE+2);memset(longline,'x',MAX_LINE_SIZE+1);longline[MAX_LINE_SIZE+1]='\n';push(longline,MAX_LINE_SIZE+2);
        } else return 2;
        atomic_store(&done_reading,1);worker_fn(NULL);
        fflush(output);fseek(output,0,SEEK_SET);char result[32]={0};size_t length=fread(result,1,sizeof(result),output);
        if(!strcmp(argv[1],"eof")) assert(length==0 && atomic_load(&total_seen)==0);
        else if(!strcmp(argv[1],"borrow")) assert(length==6 && !memcmp(result,"omega\n",6));
        else if(!strcmp(argv[1],"long")) assert(length==2 && !memcmp(result,"x\n",2));
        else assert(length==6 && !memcmp(result,"alpha\n",6));
        fclose(output);
    }
    puts("confirmed");return 0;
}
'''
source = F / 'original-functions-controls.c'
source.write_text(header + body + driver)
binary = Path('/private/tmp/rrrah-deduplicatz-controls-20261014')
assert not binary.exists()
build = subprocess.run(['cc', '-std=gnu11', '-O1', '-Wall', '-pthread', str(source), '-o', str(binary)], capture_output=True, text=True)
(F / 'build.log').write_text(build.stdout + build.stderr)
assert build.returncode == 0, build.stderr
rows = []
for case in ['zero', 'probe', 'rehash', 'duplicate', 'eof', 'split', 'borrow', 'long']:
    run = subprocess.run([str(binary), case], capture_output=True, text=True, timeout=20)
    (F / (case + '.log')).write_text(run.stdout + run.stderr)
    assert run.returncode == 0, (case, run.stderr)
    rows.append(dict(case=case, returncode=run.returncode))
report = dict(status='eight_original_function_controls_confirmed', repository='x-stp/deduplicatz', commit='2840c76c6a03615b7c7d7edf06eca54511db73fc', rows=rows, limits='Unchanged original hashset, queue and worker functions. Controlled byte hash substitutes xxHash for worker framing controls; direct hashset controls use explicit hashes. Single-thread execution; no real io_uring, O_DIRECT, concurrent races, real xxHash collisions or full CLI qualification. Rehash control constructs a valid-size occupied table directly. Borrow control models buffer reuse deterministically, not actual async kernel execution.', pins={str(p.relative_to(R)):hashlib.sha256(p.read_bytes()).hexdigest() for p in [E / names[n] for n in names] + [source, F / 'build.log', *[F / (row['case'] + '.log') for row in rows]]})
(F / 'audit.json').write_text(json.dumps(report, indent=2) + '\n')
print(report['status'])

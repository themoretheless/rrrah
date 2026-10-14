#!/usr/bin/env python3
"""Original stdin/stdout CLI controls; IP text only, no network access."""
from pathlib import Path
import hashlib
import ipaddress
import json
import os
import pstats
import random
import subprocess
import sys

R=Path(__file__).resolve().parents[1];E=R/'docs/research/dedup-500/evidence/kfeldmann--cidrmerge'
S=Path('/private/tmp/rrrah-cidr286-source-20261014');S.mkdir(exist_ok=False)
names={'README.md':'00-README.md','license.txt':'01-license.txt','cidrmerge':'02-cidrmerge','pylintrc':'03-pylintrc','test':'04-test'}
for item in json.loads((E/'tree.json').read_text())['tree']:
    data=(E/names[item['path']]).read_bytes();assert hashlib.sha1(b'blob '+str(len(data)).encode()+b'\0'+data).hexdigest()==item['sha'];(S/item['path']).write_bytes(data)
O=R/'docs/research/dedup-cidr286-controls-20261014.json';assert not O.exists()
env=os.environ.copy();env.pop('DEBUG',None);env['PYTHONDONTWRITEBYTECODE']='1'
checks=[];runs=[]

def run(label,text):
    result=subprocess.run([sys.executable,str(S/'cidrmerge')],input=text,capture_output=True,text=True,env=env,timeout=15)
    runs.append(dict(label=label,input=text,returncode=result.returncode,stdout=result.stdout,stderr=result.stderr));return result

def check(name,value):
    assert value,name;checks.append(name)

sample=(S/'test').read_text();result=run('upstream_sample',sample)
check('upstream_sample_matches_documented_output',result.returncode==0 and result.stdout=='123.10.0.0/16\n123.11.12.0/23\n')
result=run('empty_comments','\n# comment\n  \n');check('empty_and_comments_emit_nothing',result.returncode==0 and result.stdout=='')
result=run('duplicates','192.0.2.1\n192.0.2.1/32\n');check('duplicate_host_entries_collapse',result.returncode==0 and result.stdout=='192.0.2.1/32\n')
result=run('adjacent','192.0.2.0/25\n192.0.2.128/25\n');check('adjacent_halves_merge',result.returncode==0 and result.stdout=='192.0.2.0/24\n')
result=run('contained','192.0.2.0/24\n192.0.2.128/25\n');check('contained_network_deduplicates',result.returncode==0 and result.stdout=='192.0.2.0/24\n')
for label,text in [('unaligned','192.0.2.1/24\n'),('invalid_mask','192.0.2.0/33\n'),('ipv6','2001:db8::/32\n')]:
    result=run(label,text);check(label+'_refused_without_output',result.returncode==1 and result.stdout=='' and 'ERROR:' in result.stderr)
for label,text in [('octet_overflow','10.0.0.256'),('negative_octet','10.0.0.-1'),('top_octet_overflow','256.0.0.1')]:
    try:ipaddress.ip_address(text)
    except ValueError:pass
    else:raise AssertionError('control must be invalid IPv4')
    result=run(label,text+'\n');check(label+'_accepted_by_original_cli',result.returncode==0 and bool(result.stdout))
result=run('root_merge','0.0.0.0/1\n128.0.0.0/1\n');assert result.returncode==0 and result.stdout=='0.0.0.0/0\n'
replay=run('root_replay',result.stdout);check('valid_root_output_cannot_be_read_back',replay.returncode==1 and replay.stdout=='')
result=run('late_bad_input','192.0.2.0/24\nbad input\n');check('late_invalid_input_emits_no_partial_networks',result.returncode==1 and result.stdout=='')
profile=S/'sample.profile';result=subprocess.run([sys.executable,'-m','cProfile','-o',str(profile),str(S/'cidrmerge')],input=sample,capture_output=True,text=True,env=env,timeout=15);assert result.returncode==0
stats=pstats.Stats(str(profile));calls=[value[1] for key,value in stats.stats.items() if key[2]=='__cidr_to_range'];assert len(calls)==1
check('debug_disabled_still_parses_each_input_twice',calls[0]==2*len(sample.splitlines()))
rng=random.Random(173);oracle=[];base=int(ipaddress.IPv4Address('192.0.2.0'))
for index in range(64):
    networks=[]
    for _ in range(rng.randrange(1,17)):
        prefix=rng.randrange(28,33);step=1<<(32-prefix);offset=rng.randrange(256)//step*step;networks.append(ipaddress.IPv4Network((base+offset,prefix)))
    expected=[str(network) for network in ipaddress.collapse_addresses(networks)]
    result=run('oracle_'+str(index),'\n'.join(map(str,networks))+'\n');assert result.returncode==0 and result.stdout.splitlines()==expected,(index,expected,result.stdout,result.stderr)
    oracle.append(dict(index=index,inputs=len(networks),outputs=len(expected)))
assert len(checks)==14
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
O.write_text(json.dumps(dict(status='fourteen_named_cli_controls_and64_small_valid_oracles_complete',repository='kfeldmann/cidrmerge',commit='8ed8025d594664fa5644dd9c6279ca8be5c7344d',named_checks=checks,named_count=len(checks),oracle_cases=oracle,seed=173,runs=runs,pins={str(S/name):sha(S/name) for name in names},scope='Unchanged complete original CLI on Python3, original upstream sample plus synthetic IPv4 text. Independent standard ipaddress collapse oracle64small valid sets. No network requests, large-scale performance, memory bound, cancellation or file/image identity qualification. Domain rejected; no production adoption.'),indent=2)+'\n');print(len(checks),'named controls and',len(oracle),'oracle cases confirmed')

#!/usr/bin/env python3
"""Execute pinned upstream helpers with explicit synthetic adapters; no image/DB/CLI claim."""
from pathlib import Path
import hashlib,json,subprocess,random
R=Path(__file__).resolve().parents[1];D=R/'docs/research';E=D/'dedup-500/evidence/jhnc--findimagedupes';F=Path('/private/tmp/rrrah-findimagedupes49-controls-v2-20261014');F.mkdir(exist_ok=False)
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
s=(E/'02-findimagedupes').read_text();assert sha(E/'02-findimagedupes')=='d42d68367c506ece1f9dd5da21a5b7575903473b8935aa4d0a6c5cc1700e3617'
pins={p.name:sha(p)for p in E.iterdir()if p.is_file()}
c=s.split('__C__\n',1)[1]
pre=r'''
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
typedef struct SV {void *ptr; int ref;} SV;
typedef unsigned HE;
typedef struct HV {unsigned n,pos; SV vals[8]; HE current;} HV;
static unsigned hv_iterinit(HV *h){h->pos=0;return h->n;}
static HE *hv_iternext(HV *h){h->current=h->pos++;return &h->current;}
static SV *hv_iterval(HV *h,HE *e){return &h->vals[*e];}
#define SvROK(s) ((s)->ref)
#define SvRV(s) ((s)->ptr)
#define SvPV(s,l) ((char*)(s)->ptr)
#define croak(...) do {fprintf(stderr,__VA_ARGS__);exit(42);}while(0)
static long out[1024];static unsigned count;
#define Inline_Stack_Vars
#define Inline_Stack_Void return
#define Inline_Stack_Reset count=0
#define newSViv(x) ((long)(x))
#define sv_2mortal(x) (x)
#define Inline_Stack_Push(x) out[count++]=(x)
#define Inline_Stack_Done
'''
post=r'''
int main(int argc,char **argv){
 unsigned nnew=strtoul(argv[1],0,10),nold=strtoul(argv[2],0,10),threshold=strtoul(argv[3],0,10),limit=strtoul(argv[4],0,10);
 HV nh={0},oh={0};nh.n=nnew;oh.n=nold;
 for(unsigned i=0;i<nnew+nold;i++){
  unsigned len=argc>5 && !strcmp(argv[5],"short")?1:32;unsigned char *p=malloc(len);
  if(len==32){for(unsigned j=0;j<32;j++){unsigned z;sscanf(argv[5+i]+2*j,"%2x",&z);p[j]=(unsigned char)z;}}else p[0]=0;
  SV v={p,0};if(i<nnew)nh.vals[i]=v;else oh.vals[i-nnew]=v;
 }
 SV ns={&nh,1},os={&oh,1};diffbits(&os,&ns,threshold,limit);
 printf("[");for(unsigned i=0;i<count;i++)printf("%s%ld",i?",":"",out[i]);puts("]");
 for(unsigned i=0;i<nnew;i++)free(nh.vals[i].ptr);for(unsigned i=0;i<nold;i++)free(oh.vals[i].ptr);return 0;
}
'''
p=F/'adapter.c';p.write_text(pre+c+post)
records=[]
def run(cmd,label):
 r=subprocess.run(cmd,capture_output=True,text=True);o=F/(label+'.stdout');e=F/(label+'.stderr');o.write_text(r.stdout);e.write_text(r.stderr);records.append(dict(label=label,command=cmd,returncode=r.returncode,stdout=str(o),stderr=str(e),stdout_sha256=sha(o),stderr_sha256=sha(e)));return r
bins={}
for name,flags in [('zero',['-ftrivial-auto-var-init=zero']),('pattern',['-ftrivial-auto-var-init=pattern']),('asan',['-ftrivial-auto-var-init=zero','-fsanitize=address'])]:
 b=F/name;r=run(['clang','-O0',*flags,str(p),'-o',str(b)],'build-'+name);assert r.returncode==0,r.stderr;bins[name]=b
rng=random.Random(49);cases=[]
for index in range(24):
 nnew=index%4;nold=(index//4)%3;data=[rng.randbytes(32)for _ in range(nnew+nold)]
 if len(data)>1 and index%2==0:data[1]=data[0]
 threshold=[0,1,25,128,255,256][index%6];limit=index%2
 expected=[]
 for i in range(nnew if limit else max(0,len(data)-1)):
  for j in range(i+1,len(data)):
   bits=sum(bin(a^b).count('1')for a,b in zip(data[i],data[j]))
   if bits<=threshold:expected.extend([i,j,bits])
 r=run([str(bins['zero']),str(nnew),str(nold),str(threshold),str(limit),*[v.hex()for v in data]],f'zero-{index:02}');assert r.returncode==0 and json.loads(r.stdout)==expected,(index,r.stdout,expected)
 cases.append(dict(index=index,new=nnew,old=nold,threshold=threshold,limit=limit,expected=expected,fingerprints=[v.hex()for v in data]))
for name in ['zero','pattern']:
 r=run([str(bins[name]),'2','0','0','0','00'*32,'00'*32],name+'-identical');assert r.returncode==0
 if name=='zero':assert json.loads(r.stdout)==[0,1,0]
 else:assert json.loads(r.stdout)!=[0,1,0]
r=run([str(bins['asan']),'2','0','0','0','short'],'asan-short-fingerprint');assert r.returncode!=0 and 'heap-buffer-overflow'in r.stderr
# Run original threshold conversion block under core Perl, without loading image/DB/Inline dependencies.
a=s.index('if (my ($thres_val, $thres_unit)');b=s.index('for (map {split',a);block=s[a:b]
thresholds=[('90%',25),('100%',0),('0%',256),('25b',25),('256b',256),('0b',0),('12.9b',12),('90',25),(' 90% ',25),('101%',-2),('-1b',-1),('NaN',-1),('1B',-1)]
perl='use strict;use warnings;my $threshold=$ARGV[0];my @errors;sub mkerr{push @errors,@_;}\n'+block+'\nprint "$threshold ",scalar(@errors),"\\n";'
pp=F/'threshold.pl';pp.write_text(perl)
for i,(v,expected)in enumerate(thresholds):
 r=run(['perl',str(pp),v],f'threshold-{i:02}');actual,error=map(int,r.stdout.split());assert r.returncode==0 and actual==expected and error==int(expected<0 or expected>256),(v,r.stdout,expected)
# Original grouping helper; synthetic diffbits adapter returns a chain, not a clique. Generate script only.
a=s.index('sub finddupes {');b=s.index('# ======================================================================',a);group=s[a:b]
gp=F/'groups.pl';gp.write_text('use strict;use warnings;my %fpcache;my %filelist=(A=>"",B=>"",C=>"");my $threshold=1;my $add=0;my $collection;my $script=$ARGV[0];my $program;my $scriptFile;my @scriptCode;my $scriptHeader="#!/bin/sh\\nVIEW(){ :; }\\nEND(){ :; }\\n";my $collectionHeader="";my $collectionFooter="";sub diffbits{return(0,1,1,1,2,1);}sub nqdie{die @_};sub nqwarn{warn @_};sub printScriptFile{die "not used"};\n'+group+'\nfinddupes();\n')
gout=F/'groups.sh';r=run(['perl',str(gp),str(gout)],'chain-group');assert r.returncode==0,r.stderr
text=gout.read_text();assert all("'"+n+"'"in text for n in ['A','B','C'])and text.count('VIEW \\')==1
assert run(['/bin/sh','-n',str(gout)],'generated-script-syntax').returncode==0
assert all(sha(E/n)==h for n,h in pins.items())
d=dict(status='verified_bounded_original_helper_controls',repository='jhnc/findimagedupes',commit='c4920cce0735039b78cd9784e59dd21f0bd3a49a',source_pins=pins,producer_sha256=sha(__file__),upstream_c_sha256=hashlib.sha256(c.encode()).hexdigest(),adapter=str(p),adapter_sha256=sha(p),binaries={k:dict(path=str(v),sha256=sha(v))for k,v in bins.items()},records=records,scalar_cases=cases,threshold_cases=thresholds,findings=['lookup[0] is read before initialization: identical fingerprints match under compiler zero initialization but fail under pattern initialization; deterministic diagnostic of undefined behavior, not claim every production run fails','ASan confirms original memcpy reads beyond a synthetic one-byte fingerprint; no fingerprint length validation','Original Perl grouping merges chain edges into one3member group; pairwise equality is not proved','Threshold parser accepts fractional bit values by truncation and rejects uppercase B'],scope='Original C function unmodified; explicit minimal SV/HV/Inline-stack adapters replace Perl runtime, deterministic compiler stack-init modes expose uninitialized lookup. Scalar oracle24cases,13actual Perl threshold cases, original grouping helper1chain and script syntax only. No GraphicsMagick, fingerprint image pipeline, DB_File transaction, full CLI, upstream test suite, real/private media or universal performance proof. Generated shell is parsed only, not executed. No GPL source adopted in library.')
O=D/'dedup-findimagedupes49-controls-20261014.json';assert not O.exists();O.write_text(json.dumps(d,indent=2)+'\n');print(O,len(records),'recorded commands')

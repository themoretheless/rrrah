"""Geometric filter support/count audit, not a luminance or ordinal oracle."""
import argparse,ast,hashlib,json,math
from pathlib import Path
from dedup_jpeg_domain import oriented_dimensions
digest=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
helper=Path(__file__).with_name('verify-dedup-rank-region-real.py')
namespace={'math':math};tree=ast.parse(helper.read_text());functions=[v for v in tree.body if isinstance(v,ast.FunctionDef) and v.name in ['inverse','project']];assert len(functions)==2
exec(compile(ast.Module(body=functions,type_ignores=[]),str(helper),'exec'),namespace)
inverse,project=namespace['inverse'],namespace['project']

def window(mapping,x,y,radius,target_size,source_size,fast=True):
    if fast:
        corners=[(x-radius,y-radius),(x+radius,y-radius),(x-radius,y+radius),(x+radius,y+radius)]
        if all(0<=px<target_size[0] and 0<=py<target_size[1] for px,py in corners):
            denominators=[mapping[2][0]*px+mapping[2][1]*py+mapping[2][2] for px,py in corners]
            if min(denominators)>0 or max(denominators)<0:
                mapped=[project(mapping,px,py) for px,py in corners]
                # A projective image of a rectangle without a horizon is convex.
                # Keep a numerical margin; enumerate ambiguous boundaries.
                if all(1e-7<p[0]<source_size[0]-1-1e-7 and 1e-7<p[1]<source_size[1]-1-1e-7 for p in mapped):
                    return True,5*(2*radius+1)**2
    reads=0
    for py in range(y-radius,y+radius+1):
        for px in range(x-radius,x+radius+1):
            if not(0<=px<target_size[0] and 0<=py<target_size[1]):return False,reads
            reads+=5;p=project(mapping,px,py)
            if not(0<=p[0] and p[0]+1<source_size[0] and 0<=p[1] and p[1]+1<source_size[1]):return False,reads
    return True,reads

def counts(mapping,domain,opposite,target_size,source_size,radius):
    x,y,w,h=domain;ox,oy,ow,oh=opposite;valid=reads=0
    offsets=[(0,0),(-8,-8),(0,-8),(8,-8),(-8,0),(8,0),(-8,8),(0,8),(8,8)]
    for py in range(y,y+h):
        for px in range(x,x+w):
            center=project(mapping,px,py)
            if not(ox<=center[0]<ox+ow and oy<=center[1]<oy+oh):continue
            for dx,dy in offsets:
                complete,used=window(mapping,px+dx,py+dy,radius,target_size,source_size)
                reads+=used
                if not complete:break
            else:valid+=1
    return valid,reads

def verify(report,output):
    assert not output.exists();r=json.loads(report.read_text());assert r['status']=='complete_native_filtered_rank_diagnostic' and r['required_cases']==len(r['rows'])==36
    assert all(digest(k)==v for k,v in r['input_hashes'].items())
    base=report.parent;original_path=base/'dedup-rank-region-real.json';original=json.loads(original_path.read_text());audit=json.loads((base/'dedup-rank-region-real-audit.json').read_text());assert audit['status']=='verified_rank_real_membership_and_taps' and audit['input_hashes'][str(original_path)]==digest(original_path)
    root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');prepared=root/'prepared.json';manifest=json.loads(prepared.read_text());summaries=[]
    for index,row in enumerate(r['rows']):
        prior=original['rows'][index%12];radius=[0,3,8][index//12]
        assert row['filter_radius']==radius and all(row[k]==prior[k] for k in ['case','source','query','input']) and row['returncode']==0
        values=list(map(float,Path(row['input']).read_text().split()));assert len(values)==17
        h=[values[i:i+3] for i in range(0,9,3)];inv=inverse(h);domains=[list(map(int,values[9:13])),list(map(int,values[13:]))];dims=[]
        for path,split in [(Path(row['source']),'original'),(Path(row['query']),'strong')]:
            item=next(v for v in manifest['images'][split] if v['filename']==path.name);assert digest(path)==item['source_sha256'];dims.append(oriented_dimensions(path,item['source_size']))
        e=row['evidence'];assert e==json.loads(row['stdout']) and e['managed_used']==0 and 0<=e['managed_peak']<=512*1024*1024 and len(e['directions'])==2
        if radius==0:assert e==prior['evidence']
        fractions=[]
        for direction_index,(d,domain,opposite,mapping) in enumerate(zip(e['directions'],domains[::-1],domains,[inv,inverse(inv)])):
            assert d['status']=='ok';valid,reads=counts(mapping,domain,opposite,dims[1-direction_index],dims[direction_index],radius)
            assert d['sites']==domain[2]*domain[3]<=100000 and d['valid_sites']==valid and d['pixel_reads']==reads<=d['sites']*45*(2*radius+1)**2
            assert all(type(d[k])is int for k in ['sites','valid_sites','pixel_reads','informative_pairs','agreeing_pairs'])
            assert 1000<=d['informative_pairs']<=valid*8 and 0<=d['agreeing_pairs']<=d['informative_pairs']
            fractions.append(d['agreeing_pairs']/d['informative_pairs'])
        summaries.append({'case':row['case'],'filter_radius':radius,'fractions':fractions})
        print(radius,row['case'],'verified',flush=True)
    assert all(digest(k)==v for k,v in r['input_hashes'].items())
    output.write_text(json.dumps({'status':'verified_filtered_rank_real_membership_and_taps','cases':36,'directions':72,'summary':summaries,'input_hashes':{str(p):digest(p) for p in [report,Path(__file__),helper,prepared,Path(__file__).with_name('dedup_jpeg_domain.py')]},'scope':'Exact twelve-case/model/domain membership, full filter0 evidence parity and independent geometric valid-site/read counts for all three filter sizes. Convex support fast path has numerical margin and exhaustive boundary fallback. Contrast/ordinal agreement bounded only; no pixel/luminance oracle, automatic classifier or broad precision.'},indent=2)+'\n')

if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('report',type=Path);parser.add_argument('output',type=Path);a=parser.parse_args();verify(a.report,a.output)

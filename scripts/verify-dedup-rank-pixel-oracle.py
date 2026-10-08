"""Independent NumPy warp/window/ordinal math on exact native decoded pixels."""
import ast,hashlib,json,math
from pathlib import Path
import numpy as np
base=Path('docs/research');digest=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
reference=base/'dedup-filtered-rank-real.json';r=json.loads(reference.read_text());assert r['status']=='complete_native_filtered_rank_diagnostic' and len(r['rows'])==36
dump_path=base/'dedup-rank-normalized-pixels.json';dumps=json.loads(dump_path.read_text());assert dumps['status']=='complete_native_normalized_pixel_dump' and len(dumps['rows'])==8
assert all(digest(k)==v for k,v in r['input_hashes'].items()) and all(digest(k)==v for k,v in dumps['input_hashes'].items())
helper=Path('scripts/verify-dedup-rank-region-real.py');namespace={};tree=ast.parse(helper.read_text());nodes=[v for v in tree.body if isinstance(v,ast.FunctionDef) and v.name=='inverse'];assert len(nodes)==1
exec(compile(ast.Module(body=nodes,type_ignores=[]),str(helper),'exec'),namespace);inverse=namespace['inverse']
magic=b'RRRAH-RANK-RGBA32-V1\n';images={};pixel_hashes={}
for row in dumps['rows']:
    assert row['returncode']==0 and digest(row['source'])==row['source_sha256']
    path=Path(row['pixels']);assert digest(path)==row['pixels_sha256'];pixel_hashes[str(path)]=row['pixels_sha256']
    with path.open('rb') as file:
        assert file.read(len(magic))==magic;w=int.from_bytes(file.read(4),'little');h=int.from_bytes(file.read(4),'little')
    assert w==row['evidence']['width'] and h==row['evidence']['height']
    data=np.memmap(path,dtype='<f4',mode='r',offset=len(magic)+8,shape=(h,w,4))
    assert np.all(data[:,:,3]==1.) and np.all(np.isfinite(data)) and np.min(data[:,:,:3])>=0 and np.max(data[:,:,:3])<=1
    luma=np.asarray(data[:,:,0],dtype=np.float64)*.2126+np.asarray(data[:,:,1],dtype=np.float64)*.7152+np.asarray(data[:,:,2],dtype=np.float64)*.0722
    images[row['source']]=luma

def coordinates(h,x,y):
    denominator=h[2][0]*x+h[2][1]*y+h[2][2]
    assert np.all(np.isfinite(denominator)) and np.all(denominator!=0)
    return ((h[0][0]*x+h[0][1]*y+h[0][2])/denominator,(h[1][0]*x+h[1][1]*y+h[1][2])/denominator)

def measured(source,target,mapping,source_roi,target_roi,radius):
    sh,sw=source.shape;th,tw=target.shape;x,y,w,h=target_roi;ox,oy,ow,oh=source_roi
    pad=8+radius;x0=max(0,x-pad);y0=max(0,y-pad);x1=min(tw,x+w+pad);y1=min(th,y+h+pad)
    yy,xx=np.indices((y1-y0,x1-x0),dtype=np.float64);xx+=x0;yy+=y0
    mx,my=coordinates(mapping,xx,yy);safe=(mx>=0)&(my>=0)&(mx+1<sw)&(my+1<sh)
    ix=np.clip(np.floor(mx),0,sw-2).astype(np.int64);iy=np.clip(np.floor(my),0,sh-2).astype(np.int64);tx=mx-ix;ty=my-iy
    a=source[iy,ix];b=source[iy,ix+1];c=source[iy+1,ix];d=source[iy+1,ix+1]
    warped=(a+(b-a)*tx)*(1.-ty)+(c+(d-c)*tx)*ty;warped[~safe]=np.nan
    target_tile=target[y0:y1,x0:x1]
    if radius:
        size=2*radius+1
        # A separate window-reduction implementation, rather than the native
        # per-center/per-neighbor sequential accumulator.
        warped=np.lib.stride_tricks.sliding_window_view(warped,(size,size)).mean(axis=(-2,-1))
        target_tile=np.lib.stride_tricks.sliding_window_view(target_tile,(size,size)).mean(axis=(-2,-1))
    yy,xx=np.indices((h,w),dtype=np.int64);xx+=x;yy+=y
    cx,cy=coordinates(mapping,xx,yy);valid=(cx>=ox)&(cx<ox+ow)&(cy>=oy)&(cy<oy+oh)
    av=[];bv=[]
    for dx,dy in [(0,0),(-8,-8),(0,-8),(8,-8),(-8,0),(8,0),(-8,8),(0,8),(8,8)]:
        nx=xx+dx-x0-radius;ny=yy+dy-y0-radius
        exists=(nx>=0)&(ny>=0)&(nx<warped.shape[1])&(ny<warped.shape[0])
        clipped_x=np.clip(nx,0,warped.shape[1]-1);clipped_y=np.clip(ny,0,warped.shape[0]-1)
        va=warped[clipped_y,clipped_x];vb=target_tile[clipped_y,clipped_x]
        valid &= exists & np.isfinite(va) & np.isfinite(vb)
        av.append(va);bv.append(vb)
    informative=agreeing=0;minimum_margin=1.
    for a,b in zip(av[1:],bv[1:]):
        da=a-av[0];db=b-bv[0];usable=valid&(np.abs(da)>=.005)&(np.abs(db)>=.005)
        informative+=int(np.count_nonzero(usable));agreeing+=int(np.count_nonzero(usable&((da>0)==(db>0))))
        if np.any(valid):
            minimum_margin=min(minimum_margin,float(np.min(np.abs(np.abs(da[valid])-.005))),float(np.min(np.abs(np.abs(db[valid])-.005))))
    return {'valid_sites':int(np.count_nonzero(valid)),'informative_pairs':informative,'agreeing_pairs':agreeing,'minimum_contrast_boundary_margin':minimum_margin}

summary=[]
for row in r['rows']:
    values=list(map(float,Path(row['input']).read_text().split()));h=[values[i:i+3] for i in range(0,9,3)];inv=inverse(h);ar=list(map(int,values[9:13]));br=list(map(int,values[13:]));a=images[row['source']];b=images[row['query']];directions=[]
    for native,source,target,mapping,sr,tr in zip(row['evidence']['directions'],[a,b],[b,a],[inv,inverse(inv)],[ar,br],[br,ar]):
        assert native['status']=='ok';actual=measured(source,target,mapping,sr,tr,row['filter_radius'])
        for key in ['valid_sites','informative_pairs','agreeing_pairs']:assert actual[key]==native[key],(row['case'],row['filter_radius'],key,actual,native)
        directions.append(actual)
    summary.append({'case':row['case'],'filter_radius':row['filter_radius'],'directions':directions});print(row['filter_radius'],row['case'],'exact ordinal parity',flush=True)
assert all(digest(k)==v for k,v in pixel_hashes.items()) and all(digest(k)==v for k,v in r['input_hashes'].items()) and all(digest(k)==v for k,v in dumps['input_hashes'].items())
output=base/'dedup-rank-pixel-oracle-audit.json';assert not output.exists();paths=[reference,dump_path,Path(__file__),helper]
output.write_text(json.dumps({'status':'verified_independent_rank_pixel_math','cases':36,'directions':72,'numpy_version':np.__version__,'summary':summary,'input_hashes':{str(p):digest(p) for p in paths},'pixel_hashes':pixel_hashes,'scope':'Independent NumPy luminance, projective warp, bilinear interpolation, box-window means and ordinal counts agree exactly on72 directions. Pixel normalization comes from shared native decoder and is not independently qualified here. Counts on fixed supplied models do not establish copy admission or broad precision.'},indent=2)+'\n')

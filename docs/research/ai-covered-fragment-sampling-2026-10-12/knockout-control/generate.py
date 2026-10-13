from pathlib import Path
import re,json,subprocess
root=Path.cwd();out=root/'docs/research/ai-covered-fragment-sampling-2026-10-12/knockout-control';out.mkdir(exist_ok=True)
lines=(out.parent/'spatial-point-state-real.log').read_text().splitlines();paints=[];opacity=None
for line in lines:
 if 'REAL_AI_POINT_STATE' not in line:continue
 if 'depth=3 kind=begin ' in line:
  m=re.search(r'options=Some\(\(false, false, ([0-9.]+), false, Normal\)\)',line)
  opacity=float(m[1]) if m else None
 if 'depth=3 kind=retained ' in line and opacity is not None:
  color=json.loads(re.search(r'native=Some\((\[.*\])\)',line)[1]);paints.append((opacity,color))
base=[0.5443226099014282,0.21231108903884888,0.7983059883117676,0.009295190684497356]
def pdf(name,items,ko):
 objects=[]
 def add(value):objects.append(value.encode() if isinstance(value,str) else value);return len(objects)
 def stream(attrs,body):return f'<< {attrs} /Length {len(body.encode())} >>\nstream\n{body}\nendstream'
 add('<< /Type /Catalog /Pages 2 0 R >>');add('<< /Type /Pages /Kids [3 0 R] /Count 1 >>');add('');add('')
 names=[];forms=[]
 for i,(alpha,color) in enumerate(items):
  g=add(f'<< /Type /ExtGState /ca {alpha:.17g} /CA {alpha:.17g} /BM /Normal >>')
  f=add(stream('/Type /XObject /Subtype /Form /BBox [0 0 8 8] /Group << /S /Transparency /CS /DeviceCMYK /I false /K false >>', ' '.join(map(str,color))+' k 0 0 8 8 re f'))
  names.append(f'/G{i} {g} 0 R');forms.append(f'/F{i} {f} 0 R')
 body='\n'.join(f'q /G{i} gs /F{i} Do Q' for i in range(len(items)))
 group=add(stream('/Type /XObject /Subtype /Form /BBox [0 0 8 8] /Group << /S /Transparency /CS /DeviceCMYK /I false /K '+str(ko).lower()+' >> /Resources << /ExtGState << '+' '.join(names)+' >> /XObject << '+' '.join(forms)+' >> >>',body))
 objects[2]=f'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 8 8] /Group << /S /Transparency /CS /DeviceCMYK /I false /K false >> /Resources << /XObject << /Fade {group} 0 R >> >> /Contents 4 0 R >>'.encode()
 objects[3]=stream('', ' '.join(map(str,base))+' k 0 0 8 8 re f /Fade Do').encode()
 data=bytearray(b'%PDF-1.7\n');offsets=[0]
 for i,o in enumerate(objects,1):offsets.append(len(data));data.extend(f'{i} 0 obj\n'.encode()+o+b'\nendobj\n')
 start=len(data);data.extend(f'xref\n0 {len(objects)+1}\n0000000000 65535 f \n'.encode());data.extend(''.join(f'{x:010d} 00000 n \n' for x in offsets[1:]).encode());data.extend(f'trailer << /Size {len(objects)+1} /Root 1 0 R >>\nstartxref\n{start}\n%%EOF\n'.encode());path=out/(name+'.pdf');path.write_bytes(data)
 args=['/tmp/rrrah-poppler-profile-oracle-20261012/build/utils/pdftoppm','-r','72','-singlefile','-defaultcmykprofile',str(root/'vendor/hayro-interpret/assets/CGATS001Compat-v2-micro.icc'),str(path),str(out/name)]
 subprocess.run(args,check=True)
for name,items,ko in [('ko-all',paints,True),('ko-last',paints[-1:],True),('nonko-all',paints,False)]:pdf(name,items,ko)
(out/'inputs.json').write_text(json.dumps({'base':base,'paints':paints},indent=2)+'\n')
print('paints',len(paints))

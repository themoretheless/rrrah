from pathlib import Path
import os,subprocess
root=Path.cwd();cache=root/'.dedup-work/rubyhash401-20261021';env=os.environ.copy();gem=root/'.dedup-work/rubyhash401-gems-20261021';env['GEM_HOME']=str(gem);defaults=subprocess.check_output(['ruby','-e','puts Gem.path'],text=True).splitlines();env['GEM_PATH']=':'.join([str(gem),*defaults]);base=root/'docs/research/dedup-500/rubyhash401-controls-20261021'
with (base/'pure-original-specs.log').open('w') as out:
 subprocess.run(['ruby','-I'+str(cache/'lib'),'-rrspec/autorun','-rimage_hash',str(cache/'spec/image_hash_spec.rb'),'--options','/dev/null','--example','.hex_to_binary','--example','.binary_to_hex','--example','.binary_distance','--example','.distance'],env=env,stdout=out,stderr=subprocess.STDOUT,check=True)
with (base/'run.log').open('w') as out:subprocess.run(['ruby','-I'+str(cache/'lib'),str(base/'controls.rb')],env=env,stdout=out,stderr=subprocess.STDOUT,check=True)
for name in ['run.log','pure-original-specs.log']:
 p=base/name;p.write_text('\n'.join(x.rstrip() for x in p.read_text().splitlines()).rstrip()+'\n')

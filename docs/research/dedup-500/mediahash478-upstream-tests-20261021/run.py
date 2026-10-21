from pathlib import Path
import sys,importlib.metadata,json
root=Path.cwd();sys.path[:0]=[str(root/'.dedup-work/mediahash478-extra-deps-20261021'),str(root/'.dedup-work/mediahash478-pytest-20261021'),'/Users/themoretheless/.codex/tmp/rrrah-mediahash-deps-20261013',str(root/'.dedup-work/mediahash478-20261021/src')]
import pytest
print(json.dumps({n:importlib.metadata.version(n) for n in ['pytest','av','Pillow','ImageHash','numpy','scipy','PyWavelets','context-utils']}),flush=True)
rc=pytest.main(['-q','-o','addopts=--import-mode=importlib','-p','no:cacheprovider',str(root/'.dedup-work/mediahash478-20261021/__test__')]);print('PYTEST_EXIT',rc,flush=True);sys.exit(rc)

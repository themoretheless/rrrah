from pathlib import Path
import hashlib,json,importlib.metadata as md
W=Path('/private/tmp/rrrah-code33-native-20261015');M=W/'modules';E=Path('/Users/themoretheless/Documents/Sources/rrrah/docs/research/code33-native-controls-20261015');h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();versions=sorted([dict(name=d.metadata['Name'],version=d.version) for d in md.distributions(path=[str(M)])],key=lambda d:d['name'].lower());assert len(versions)==10;files=[]
for p in sorted(M.rglob('*')):
 if p.is_file() and '__pycache__' not in p.parts and p.suffix!='.pyc':files.append(dict(path=str(p),bytes=p.stat().st_size,sha256=h(p)))
out=E/'dependency-pins.json';assert not out.exists();out.write_text(json.dumps(dict(distributions=versions,installed_payloads=files,scope='All actual10 isolatedinstalled wheelcohort distributions bytepinned excludinggeneratedpycache. Upstreamlowerbounds/unlockedrequirements, not historicalDocker3.7 ortransitive source/build/package qualification. No project/systemdependency changes.'),indent=2)+'\n');print(len(versions),len(files),'installed payloads pinned')

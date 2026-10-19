"""Selected ordinary scanner controls on whole original modules and owned fixtures."""
from pathlib import Path
import hashlib
import importlib.metadata
import json
import sys
import zipfile
import warnings

ROOT = Path(__file__).resolve().parents[1]
WORK = ROOT / '.dedup-work/fizz457-20261019'
SOURCE = WORK / 'source'
FIXTURES = WORK / 'fixtures'
FIXTURES.mkdir(exist_ok=False)
sys.path.insert(0, str(SOURCE))
from core.scanner import DuplicateScanner
from core.analyzers import FileHasher, ImageAnalyzer, VideoAnalyzer
from core.deletion import DeletionManager
from PIL import Image

first = FIXTURES / 'first.png'
second = FIXTURES / 'second.png'
image = Image.new('RGB', (32,32))
image.putdata([(x*7, y*7, (x+y)*3) for y in range(32) for x in range(32)])
image.save(first)
second.write_bytes(first.read_bytes())
for name in ['first.bin','second.bin']:
    (FIXTURES/name).write_bytes(b'ordinary binary content\x00\xff')
options = {'exact_match':True,'similar_images':False,'similar_videos':False,'thread_count':2}
scanner = DuplicateScanner(options)
scanner.scan_directory(str(FIXTURES))
assert set(scanner.files) == {str(first),str(second)}
groups = scanner.find_duplicates()
assert len(groups) == 1 and {x['path'] for x in groups[0]} == {str(first),str(second)}
assert FileHasher().hash_file(str(first)) == hashlib.sha256(first.read_bytes()).hexdigest()
scanner.scan_directory(str(FIXTURES))
assert len(scanner.files) == 4  # repeated scans append paths, no reset/dedup
merged = scanner._merge_duplicate_groups([[{'path':'a'},{'path':'b'}],[{'path':'b'},{'path':'c'}]])
assert merged == [[{'path':'a'},{'path':'b'}]]  # overlapping group loses c
analyzer = ImageAnalyzer()
left = analyzer.analyze(str(first)); right = analyzer.analyze(str(second))
assert left is not None and right is not None and analyzer.compare(left,right,100)
assert VideoAnalyzer().compare({'frame_hashes':['a','b'],'duration':10},
                               {'frame_hashes':['a','b','c'],'duration':10},100)
backup = FIXTURES / 'backups'
manager = DeletionManager({'backup_directory':str(backup),'use_trash':True})
same_names = []
for folder, content in [('one',b'first'),('two',b'second')]:
    p = FIXTURES/folder/'same.png';p.parent.mkdir();p.write_bytes(content);same_names.append(str(p))
with warnings.catch_warnings(record=True) as emitted:
    warnings.simplefilter('always')
    archive = manager._create_backup(same_names)
with zipfile.ZipFile(archive) as z:
    assert z.namelist() == ['same.png','same.png']
    assert [z.read(info) for info in z.infolist()] == [b'first',b'second']
assert any('Duplicate name' in str(w.message) for w in emitted)
assert all(Path(p).exists() for p in same_names)  # no deletion method invoked
report = ROOT / 'docs/research/dedup-500/fizz457-selected-runtime-20261019.json'
assert not report.exists()
report.write_text(json.dumps({
    'status':'passed_selected_original_module_controls',
    'repository':'Fizzolas/duplicate-file-finder','commit':'18d1b587e7bc13b6fe610da5efd8f812887251e3',
    'findings':['Actual public exact scan finds two identical PNGs with real SHA256',
                'Directory scanner excludes both identical .bin files by its media-only suffix list',
                'Repeated scan appends paths again:2 becomes4',
                'Original overlapping group merge returns[a,b] and drops c from[b,c]',
                'Actual real Pillow/imagehash image identity compares equal at100',
                'Original video signature comparator accepts a matching prefix at100; this is signature-only, no video decode claim',
                'Backup stores two distinct source files under the same basename; ZIP contains duplicate same.png entries and emits warning'],
    'dependencies':{name:importlib.metadata.version(name) for name in ['opencv-python','ImageHash','Pillow','numpy','Send2Trash']},
    'source_pins':{str(p.relative_to(SOURCE)):hashlib.sha256(p.read_bytes()).hexdigest() for p in SOURCE.rglob('*') if p.is_file() and '__pycache__' not in p.parts},
    'producer_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    'limits':['Whole original modules imported with actual dependencies, no body extraction or fake provider',
              'Only owned synthetic fixture files used; no file deletion method or external messages invoked',
              'No GUI/PyQt6/package/release/actual-video decoder/all-format/performance qualification',
              'No upstream tests exist in pinned repository; these are independently written selected controls',
              'External code is research evidence; no production adoption']},indent=2)+'\n')
print(report)

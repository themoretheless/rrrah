import sys,tempfile,json
from pathlib import Path
sys.path[:0]=[str(Path('.dedup-work/mediadedupe444-20261021').resolve()),str(Path('.dedup-work/mediahash478-extra-deps-20261021').resolve())]
import engine
out={}
with tempfile.TemporaryDirectory() as tmp:
 root=Path(tmp);report=root/'report.html'
 group=[dict(path=str(root/'small.png'),filename='small.png',width=1,height=1,size=10,mtime=1),dict(path=str(root/'large.png'),filename='large.png',width=10,height=10,size=100,mtime=1)]
 engine.create_html_report(root,[],[group],[],report)
 text=report.read_text();assert 'Wasted storage (approx.):</b> 100.0 B' in text
 assert text.index('<p class="filename">large.png')<text.index('<p class="filename">small.png')
 out['storage_summary_before_sort']=dict(displayed_waste_bytes=100,selected_keeper='large.png',selected_candidate='small.png',candidate_bytes=10,scope='Supplied metadata, missing image previews; original HTML report generation, no actual deletion or UI interaction')
 engine.create_html_report(root,[],[],[],report)
 text=report.read_text();assert 'Videos — Exact Duplicates (0 groups)' in text
 out['unscanned_video_section']=dict(video_arguments='all default None',contains_video_sections=True,scope='Original report helper converts None to[] before checking is not None')
Path(__file__).with_name('result.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2))

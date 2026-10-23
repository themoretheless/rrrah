from pathlib import Path
import json,importlib.metadata
from jinja2 import Environment,FileSystemLoader,select_autoescape
base=Path(__file__).resolve().parent/'intra095-20261024/inspector/templates'
env=Environment(loader=FileSystemLoader(base),autoescape=select_autoescape(['html']))
env.globals.update(url_for=lambda *a,**kw:'/owned',get_flashed_messages=lambda:[],manga_scrapers=[],hentai_scrapers=[],other_scrapers=[])
compiled=[]
for p in sorted(base.glob('*.html')):env.get_template(p.name);compiled.append(p.name)
rows=[(i,'owned-'+str(i),'member') for i in range(100)]
html=env.get_template('overcommon_file.html').render(matches={0:rows},title='owned')
assert '100 items' in html and html.count('/debug/single_item/')==100
macro=env.get_template('_macros.html').module.createReaderLink('owned',None)
assert '</%def>' in macro
single=env.get_template('single_file.html').render(rowid=7,title='owned')
assert all('/debug/scraper_resource/step_'+str(i)+'/7' in single for i in range(1,6))
print(json.dumps(dict(jinja_version=importlib.metadata.version('jinja2'),compiled_templates=compiled,detail_rows_rendered=html.count('/debug/single_item/'),detail_count_label='100 items',reader_macro_output=str(macro),single_step_links=5,scope='Original unchanged templates compiled/rendered by real Jinja2 with owned data and URL/flash globals;no Flask,HTTP,DB,browser,external assets or visual screenshot validation'),sort_keys=True))

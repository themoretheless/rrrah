import importlib.util,types,io,json
from pathlib import Path
from lxml import etree
root=Path('.dedup-work/sms134-20261024/pinned');spec=importlib.util.spec_from_file_location('original_dedupe',root/'dedupe_texts.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
a=types.SimpleNamespace(default_country_code='+1',ignore_date_milliseconds=False,ignore_whitespace_differences=False,aggressive=False)
def dedup(xml):
 tree=etree.ElementTree(etree.fromstring(xml));log=io.StringIO();out,before,after=m.deduplicate_messages_in_tree(tree,log,a);return out,log.getvalue(),before,after
original=m.read_input_xml(root/'tests/test_cases/multiple_duplicates.xml');out,before,after=m.deduplicate_messages_in_tree(original,io.StringIO(),a);assert len(out.getroot())==2 and sum(before.values())==7 and sum(after.values())==2
out,*_=dedup(b'<smses><sms date="1000" body="same" service_center="a"/><sms date="1000" body="same" service_center="b"/></smses>');assert len(out.getroot())==1
out,*_=dedup(b'<smses><mms date="1000" data="AAAA"/><mms date="1000" data="BBBB"/></smses>');assert len(out.getroot())==2
out,log,*_=dedup(b'<smses><sms date="1000" body="same"/><mms date="1000" body="same"/><sms date="1000" body="same"/></smses>');assert len(out.getroot())==2 and 'Removing sms:' in log and 'In favor of keeping mms:' in log
result={'controls_passed':4,'scope':'Full original module imported without CLI main;actual lxml6.0.2;upstream fixture and synthetic XML only;no user backups','findings':['Independent equality assertions confirm7to2 messages for multiple_duplicates fixture.','Different service_center metadata intentionally ignored in domain key;not byte identity.','Different data payload strings kept separately.','Default mode keeps SMS/MMS separately but shared log representative map can report MMS as kept representative for removed SMS.'],'cross_tag_log':log};Path(__file__).with_name('results.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))

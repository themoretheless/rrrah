import pathlib,tempfile,json,sys
sys.path.insert(0,str(pathlib.Path(__file__).parent/".dedup-work-placeholder"))
import link_cleanup,supersede,common
results=[]
with tempfile.TemporaryDirectory(prefix="autograph104-links-owned-") as tmp:
 root=pathlib.Path(tmp);(root/'good.md').write_text('# Good')
 idx=common.build_link_index(root)
 text='# Card\n## Related\n- [[good]] and [[missing]]\n'
 out,removed,kept=link_cleanup.cleanup_related_section(text,idx)
 assert removed==['missing'] and '[[missing]]' in out
 results.append({'control':'mixed-link-line-reports-removal-but-retains-link','removed_report':removed,'broken_link_retained':True})
 d=root/'folder';d.mkdir();(d/'child.md').write_text('# Child');idx=common.build_link_index(root)
 text='# Card\n## Related\n- [[./child]]\n'
 out,removed,kept=link_cleanup.cleanup_related_section(text,idx)
 assert removed==['./child'] and '## Related' not in out
 assert common.resolve_link_target('./child',idx,'folder/source.md')[0]=='folder/child'
 results.append({'control':'relative-link-missing-source-context','valid_from_source':True,'cleanup_removes_section':True})
 a=root/'old.md';b=root/'new.md';same='---\ntype: note\nupdated: 2026-01-01\n---\nBody'
 a.write_text(same);b.write_text(same);before=a.read_bytes()
 count=supersede.apply_supersede(root,[{'current':'new.md','superseded_candidates':['old.md']}])
 assert count==0 and a.read_bytes()==before
 results.append({'control':'supersede-equal-date-no-mutation','changed':count})
print(json.dumps({'passed':3,'author_controls':results,'scope':'Unchanged original functions on temporary owned files; no user vault; not upstream tests'},indent=2))

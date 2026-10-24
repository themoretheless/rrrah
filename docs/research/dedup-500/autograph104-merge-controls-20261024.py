import pathlib,tempfile,json,dedup
results=[]
with tempfile.TemporaryDirectory(prefix="autograph104-merge-owned-") as tmp:
 root=pathlib.Path(tmp)
 def card(name,content):
  p=root/name;p.write_text(content);return p
 a=card("canonical.md","---\ntags: [alpha]\n---\n# Card\n## Notes\noriginal fact\n")
 b=card("extra.md","---\ntags: [beta]\n---\n# Card\n## Notes\nadditional unique fact\n")
 changed=dedup.merge_content(a,[b]);text=a.read_text();assert not changed and "additional unique fact" not in text and "beta" not in text
 results.append({"control":"same-heading-and-equal-length-list-not-merged","changed":changed,"extra_retained":b.exists(),"scope":"merge_content only; no extra move or deletion invoked"})
 a=card("canonical.md","---\ncompany: First\nupdated: 2026-01-01\n---\n# Card\n")
 b=card("extra.md","---\ncompany: Second\nupdated: 2026-02-01\n---\n# Card\n")
 c=card("third.md","---\ncompany: Third\nupdated: 2026-03-01\n---\n# Card\n")
 dedup.merge_content(a,[b,c],['company'],today='2026-10-24');text=a.read_text();assert 'company: Second' in text and 'company: Third' not in text.split('---',2)[1] and 'company: Third' in text
 results.append({"control":"merge-date-stamp-blocks-later-extra","observed_company":"Second","third_recorded_in_history":True,"scope":"Processing February card stamps canonical October date before comparing March card"})
 a=card("links.md","```\n[[old/path]]\n```\n[[old/path#Heading]]\n[[old/path|label]]\n")
 dedup.redirect_links(root,['old/path.md'],'new/path.md');text=a.read_text();assert '```\n[[new/path]]' in text and '[[old/path#Heading]]' in text and '[[new/path|label]]' in text
 results.append({"control":"redirect-literal-and-anchor-boundaries","code_example_rewritten":True,"heading_link_unchanged":True,"display_label_preserved":True})
print(json.dumps({"author_controls":results,"passed":len(results),"scope":"Unchanged original dedup/common on owned temporary cards; original upstream tests not run; no user vault changed"},indent=2))

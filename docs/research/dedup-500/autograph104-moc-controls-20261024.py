import pathlib,tempfile,json,moc
results=[]
with tempfile.TemporaryDirectory(prefix="autograph104-moc-owned-") as tmp:
 root=pathlib.Path(tmp);p=root/'index.md'
 moc.save_moc(p,'---\ndescription: Old summary\ntype: moc\n---\n# First')
 moc.save_moc(p,'---\ndescription: New summary\ntype: moc\n---\n# Second')
 assert 'description: Old summary' in p.read_text() and '# Second' in p.read_text()
 results.append({'control':'generated-frontmatter-not-updated','old_summary_retained':True,'new_body_present':True,'scope':'Frontmatter lives outside managed block; ownership choice, not proven contract violation'})
 p.write_text(moc.MANAGED_END+'\nmanual\n'+moc.MANAGED_START);before=p.read_bytes()
 try:moc.save_moc(p,'# New')
 except ValueError:pass
 else:raise AssertionError('expected invalid marker order failure')
 assert p.read_bytes()==before
 results.append({'control':'reversed-markers-raise-without-write','exception':'ValueError','bytes_preserved':True})
 p.unlink();(root/'card.md').write_text('---\ntype: note\ndomain: personal\n---\n# Card');d=root/'MOC';d.mkdir()
 cards=moc.build_moc_data(root,{})['personal'];out=d/'MOC-personal.md';moc.save_moc(out,moc.generate_moc('personal',cards,{}))
 cards=moc.build_moc_data(root,{})['personal'];text=moc.generate_moc('personal',cards,{})
 assert any(c['path']=='MOC/MOC-personal' for c in cards) and '[[MOC/MOC-personal|' in text
 results.append({'control':'generated-moc-reenters-input','card_count_second_scan':len(cards),'self_link_generated':True})
print(json.dumps({'passed':3,'author_controls':results,'scope':'Original moc/common, temporary owned files; no user vault'},indent=2))

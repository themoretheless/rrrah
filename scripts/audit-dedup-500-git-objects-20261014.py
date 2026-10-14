#!/usr/bin/env python3
"""Check stored originals against Git object identities, not only self-recorded SHA256."""
from pathlib import Path,PurePosixPath
from collections import defaultdict
import hashlib,json
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
def object_sha(kind,data):return hashlib.sha1(kind.encode()+b' '+str(len(data)).encode()+b'\0'+data).hexdigest()
rows=[];errors=[];counts=defaultdict(int)
for path in sorted((D/'reviews').glob('*.json'),key=lambda p:int(p.stem)):
    review=json.loads(path.read_text());folder=D/'evidence'/review['repository'].replace('/','--');tree_path=folder/'tree.json';tree=json.loads(tree_path.read_text());entries=tree['tree'];by_path={e['path']:e for e in entries};children=defaultdict(list)
    for entry in entries:
        parent=str(PurePosixPath(entry['path']).parent);parent=''if parent=='.'else parent;children[parent].append(entry)
    tree_verified=0
    if not tree.get('truncated'):
        for directory in sorted(set(children)|{''},key=lambda p:(-p.count('/'),p)):
            # Each node's exact Git serialization checks declared child identities.
            ordered=sorted(children[directory],key=lambda e:(PurePosixPath(e['path']).name+('/'if e['type']=='tree'else'')).encode())
            raw=b''.join(e['mode'].lstrip('0').encode()+b' '+PurePosixPath(e['path']).name.encode()+b'\0'+bytes.fromhex(e['sha'])for e in ordered)
            expected=tree['sha']if directory==''else by_path[directory]['sha'];actual=object_sha('tree',raw)
            if actual!=expected:errors.append(dict(index=review['index'],kind='git_tree_object_mismatch',path=directory,expected=expected,actual=actual))
            else:tree_verified+=1
    verified=0;fragments=0;seen=set()
    for key in ['available_source_evidence','available_readme_license_evidence']:
        for evidence in review.get(key,[]):
            local=folder/evidence['local'];data=local.read_bytes();counts['stored_files']+=1
            assert sha(local)==evidence['sha256']
            if evidence.get('truncated'):
                fragments+=1;continue
            entry=by_path.get(evidence['path'])
            if entry is None:errors.append(dict(index=review['index'],kind='stored_path_absent_from_original_tree',path=evidence['path']));continue
            actual=object_sha('blob',data)
            if actual!=entry['sha']:errors.append(dict(index=review['index'],kind='git_blob_object_mismatch',path=evidence['path'],expected=entry['sha'],actual=actual,stored_bytes=len(data),tree_bytes=entry.get('size')))
            else:verified+=1;seen.add(evidence['path'])
    all_blobs={e['path']for e in entries if e['type']=='blob'};missing=sorted(all_blobs-seen)
    counts['verified_full_blob_files']+=verified;counts['declared_fragments']+=fragments;counts['tree_nodes_verified']+=tree_verified;counts['original_blob_paths']+=len(all_blobs);counts['repositories_all_blob_files_stored']+=not missing and not tree.get('truncated')
    rows.append(dict(index=review['index'],repository=review['repository'],commit=review['commit'],registry_sha256=sha(path),tree_json_sha256=sha(tree_path),tree_root_sha=tree['sha'],tree_truncated=bool(tree.get('truncated')),tree_nodes_verified=tree_verified,verified_full_stored_blobs=verified,declared_fragments=fragments,original_tree_blob_paths=len(all_blobs),missing_original_blob_paths=missing,all_original_blobs_stored=not missing and not tree.get('truncated')))
assert len(rows)==500 and len({r['repository']for r in rows})==500
O=D/'git-object-integrity-audit-20261014.json';assert not O.exists();O.write_text(json.dumps(dict(status='verified_bounded_stored_git_objects'if not errors else 'git_object_integrity_findings',auditor_sha256=sha(__file__),counts=dict(counts),errors=errors,rows=rows,scope='All500captured original tree inventories and2952stored registry files. Checks canonical Git tree SHA1serialization and full stored blob identities in addition to SHA256. Truncated stored excerpts explicitly excluded from complete-blob proof. This is provenance/coverage inventory, not reading/executing all original source files, source-review completion, current remote branch confirmation or proof that captured tree root belongs to each recorded commit.'),indent=2)+'\n');print(dict(counts),len(errors));print(errors[:8])

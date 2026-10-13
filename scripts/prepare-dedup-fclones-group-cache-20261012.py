#!/usr/bin/env python3
from pathlib import Path
import shutil,json,hashlib
R=Path(__file__).resolve().parents[1];T=Path('/Users/themoretheless/.codex/tmp');D=R/'docs/research/dedup-500';old=T/'rrrah-fclones-full-source-20261012';s=T/'rrrah-fclones-group-cache-controls-source-20261012';m=json.load(open(D/'fclones-hasher-source-20261012.json'));assert not s.exists();s.mkdir()
for k in m['file_hashes']:
 q=s/k;q.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(old/k,q)
src=(old/'fclones/src/group.rs').read_text();a=src.index('pub fn group_files(');b=src.index('\n/// Writes the list of groups',a);body=src[a:b];helper=body.replace('pub fn group_files(config: &GroupConfig, log: &dyn Log)', 'fn grouped_with_temp_cache(config: &GroupConfig, log: &dyn Log, cache_path: &Path)',1);needle='    let ctx = GroupCtx::new(config, log)?;';assert helper.count(needle)==1;helper=helper.replace(needle,'    let mut ctx = GroupCtx::new(config, log)?;\n    ctx.hasher.cache = Some(crate::cache::HashCache::open(cache_path, None, config.hash_fn)?);',1)
tests=r'''
#[cfg(test)]
mod rrrah_group_cache_controls {
 use super::*;
 use std::fs;
 use std::time::{Duration, UNIX_EPOCH};
 fn control(nanos:u32) {
  let temp=tempfile::tempdir().unwrap();let a=temp.path().join("a");let b=temp.path().join("b");
  fs::write(&a,b"AAAA").unwrap();fs::write(&b,b"AAAA").unwrap();
  let original_time=UNIX_EPOCH+Duration::new(1_700_000_000,123_400_100);
  fs::File::options().write(true).open(&b).unwrap().set_times(fs::FileTimes::new().set_modified(original_time)).unwrap();
  let config=GroupConfig{paths:vec![Path::from(a.clone()),Path::from(b.clone())],cache:false,..GroupConfig::default()};
  let mut log=crate::log::StdLog::new();log.no_progress=true;
  let cache=Path::from(temp.path().join("cache"));let before=FileMetadata::new(&Path::from(b.clone())).unwrap();
  let warm=grouped_with_temp_cache(&config,&log,&cache).unwrap();assert_eq!(warm.len(),1);assert_eq!(warm[0].files.len(),2);
  fs::write(&b,b"BBBB").unwrap();let edited_time=UNIX_EPOCH+Duration::new(1_700_000_000,nanos);
  fs::File::options().write(true).open(&b).unwrap().set_times(fs::FileTimes::new().set_modified(edited_time)).unwrap();
  let after=FileMetadata::new(&Path::from(b.clone())).unwrap();assert_eq!(before.file_id(),after.file_id());assert_eq!(before.len(),after.len());assert_eq!(after.modified().unwrap(),edited_time);
  if nanos!=123_400_100 {assert_ne!(before.modified().unwrap(),after.modified().unwrap());}
  assert_ne!(fs::read(&a).unwrap(),fs::read(&b).unwrap());
  let stale=grouped_with_temp_cache(&config,&log,&cache).unwrap();assert_eq!(stale.len(),1);assert_eq!(stale[0].files.len(),2);
  let fresh=group_files(&config,&log).unwrap();assert!(fresh.is_empty());
 }
 #[test] fn original_grouping_pipeline_reuses_stale_group_after_restored_mtime(){control(123_400_100);}
 #[test] fn original_grouping_pipeline_reuses_stale_group_after_submillisecond_edit(){control(123_400_900);}
}
''';control=D/'fclones-group-cache-controls-20261012.rs';assert not control.exists();control.write_text('// MIT License, Copyright (c)2020 Piotr Kolaczkowski. Original group_files adapter below; full license in fclones-controls-LICENSE-20261012.\n'+helper+tests);p=s/'fclones/src/group.rs';p.write_bytes(p.read_bytes()+b'\n'+control.read_bytes());sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();out={'status':'prepared_original_grouping_pipeline_temp_cache_adapter','snapshot':str(s),'parent_manifest':'fclones-hasher-source-20261012.json','parent_sha256':sha(D/'fclones-hasher-source-20261012.json'),'control':str(control),'control_sha256':sha(control),'file_hashes':{k:sha(s/k)for k in m['file_hashes']},'fully_read_additional':['fclones/src/group.rs'],'scope':'Original production unchanged; appended original group_files body adapter changes only function name/extra cachepath argument and supplies isolated temporary HashCache instead of default usercache. Same ordinary complete pipeline. Two independent filesystem/cached-vs-uncached controls. Not CLI defaultcache or full42file review.'};(D/'fclones-group-cache-controls-source-20261012.json').write_text(json.dumps(out,indent=2)+'\n');print('Pinned original production and isolated grouping-cache controls prepared')

// MIT License, Copyright (c)2020 Piotr Kolaczkowski. Original group_files adapter below; full license in fclones-controls-LICENSE-20261012.
fn grouped_with_temp_cache(config: &GroupConfig, log: &dyn Log, cache_path: &Path) -> Result<Vec<FileGroup<FileInfo>>, Error> {
    let spinner = log.progress_bar("Initializing", ProgressBarLength::Unknown);
    let mut ctx = GroupCtx::new(config, log)?;
    ctx.hasher.cache = Some(crate::cache::HashCache::open(cache_path, None, config.hash_fn)?);

    drop(spinner);
    let matching_files = scan_files(&ctx);

    let mut groups = match &ctx.hasher.transform {
        Some(_transform) => {
            let mut files = matching_files.into_iter().flatten().collect_vec();
            deduplicate(&mut files, |_| {});
            update_file_locations(&ctx, &mut files);
            group_transformed(&ctx, files)
        }
        _ => {
            let size_groups = group_by_size(&ctx, matching_files);
            let mut size_groups_pruned = remove_same_files(&ctx, size_groups);
            update_file_locations(&ctx, &mut size_groups_pruned);
            let prefix_len = ctx
                .config
                .max_prefix_size
                .unwrap_or_else(|| prefix_len(&ctx.devices, flat_iter(&size_groups_pruned)));
            let prefix_groups = group_by_prefix(&ctx, prefix_len, size_groups_pruned);
            let suffix_groups = group_by_suffix(&ctx, prefix_groups);
            if !ctx.config.skip_content_hash {
                group_by_contents(&ctx, prefix_len, suffix_groups)
            } else {
                suffix_groups
            }
        }
    };
    groups.par_sort_by_key(|g| Reverse((g.file_len, g.file_hash.u128_prefix())));
    groups
        .par_iter_mut()
        .for_each(|g| g.sort_by_path(&ctx.group_filter.root_paths));
    Ok(groups)
}

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

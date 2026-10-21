// Unchanged grouping functions, minimal metadata/output adapters and authored tests.
use std::collections::{HashMap,HashSet};use std::rc::Rc;
type Hash=[u8;32];const ZERO_HASH:Hash=[0;32];type PathRef=Rc<std::path::PathBuf>;type RecursivePathRef=Rc<u64>;
struct Arguments{root_paths:Vec<PathRef>,minimum_file_size:u64}
struct FileData{path:RecursivePathRef,root_path:Option<PathRef>,size:u64,content_hash:Hash,extent_hash:Hash}
type FileDatabase=Vec<FileData>;struct Output;impl Output{fn message_format(&self,_:std::fmt::Arguments){}}
fn entry(id:u64,root:&PathRef,content:Hash,extent:Hash,size:u64)->FileData{FileData{path:Rc::new(id),root_path:Some(root.clone()),size,content_hash:content,extent_hash:extent}}
pub fn build_dedupe_map (
	output: & Output,
	arguments: & Arguments,
	file_database: & FileDatabase,
) -> HashMap <RecursivePathRef, RecursivePathRef> {

	// find all unique files

	let deduplication_candidates =
		group_deduplication_candidates (
			arguments,
			& file_database);

	let unique_hash_count =
		deduplication_candidates.len ();

	output.message_format (
		format_args! (
			"Found {} unique hashes",
			unique_hash_count));

	// filter to duplicated files

	let deduplication_candidates: HashMap <Hash, Vec <usize>> =
		deduplication_candidates.into_iter ().filter (
			|& (ref _hash, ref file_data_indices)|

			file_data_indices.len () > 1

		).collect ();

	let duplicated_file_count =
		deduplication_candidates.len ();

	output.message_format (
		format_args! (
			"Found {} unique hashes with multiple instances",
			duplicated_file_count));

	// filter to files with physical extents

	let deduplication_candidates: HashMap <Hash, Vec <usize>> =
		deduplication_candidates.into_iter ().map (
			|(content_hash, file_data_indices)|

		(
			content_hash,

			file_data_indices.into_iter ().filter (
				|& file_data_index|

				file_database [file_data_index].extent_hash
					!= ZERO_HASH

			).collect ()
		)

	).filter (
		|& (ref _hash, ref file_data_indices): & (Hash, Vec <usize>)|

		file_data_indices.len () > 1

	).collect ();

	let physical_duplicated_file_count =
		deduplication_candidates.len ();

	output.message_format (
		format_args! (
			"Found {} unique hashes which can be deduplicated",
			physical_duplicated_file_count));

	// filter to files which are not deduplicated

	let deduplication_candidates: HashMap <Hash, Vec <usize>> =
		deduplication_candidates.into_iter ().filter (
			|& (ref _hash, ref file_indices)| {

		let first_file_index =
			file_indices [0];

		let ref first_file_data =
			file_database [first_file_index];

		let first_extent_hash =
			first_file_data.extent_hash;

		file_indices.iter ().any (
			|file_index|

			file_database [* file_index].extent_hash
				!= first_extent_hash

		)

	}).collect ();

	let physical_not_deduplicated_file_count =
		deduplication_candidates.len ();

	output.message_format (
		format_args! (
			"Found {} unique hashes which need deduplication",
			physical_not_deduplicated_file_count));

	// work out what to deduplicate

	let dedupe_map: HashMap <RecursivePathRef, RecursivePathRef> =
		deduplication_candidates.into_iter ().flat_map (
			|(_hash, file_data_indices)| {

		let ref first_file_data =
			file_database [
				file_data_indices [0]];

		let first_file_path =
			first_file_data.path.clone ();

		file_data_indices.into_iter ().map (
			move |file_data_index|

			(
				file_database [file_data_index].path.clone (),
				first_file_path.clone (),
			)

		)

	}).collect ();

	output.message_format (
		format_args! (
			"Found {} files to deduplicate",
			dedupe_map.len ()));

	// return

	dedupe_map

}

fn group_deduplication_candidates (
	arguments: & Arguments,
	file_database: & FileDatabase,
) -> HashMap <Hash, Vec <usize>> {

	let mut identical_files_map =
		HashMap::new ();

	let root_set: HashSet <PathRef> =
		arguments.root_paths.iter ().map (
			|root_path|
			root_path.clone ()
		).collect ();

	for (file_index, file_data)
	in file_database.iter ().enumerate () {

		if file_data.size < arguments.minimum_file_size {
			continue;
		}

		if (

			(

				file_data.root_path.is_none ()

			) || (

				file_data.root_path.is_some ()

				&& ! root_set.contains (
					& file_data.root_path.as_ref ().unwrap ().clone ())

			)

		) {
			continue;
		}

		identical_files_map.entry (
			file_data.content_hash,
		).or_insert_with (
			|| Vec::new (),
		).push (
			file_index,
		);

	}

	identical_files_map

}


#[test]fn failed_zero_content_hashes_enter_dedupe_map(){let root:PathRef=Rc::new("/photos".into());let args=Arguments{root_paths:vec![root.clone()],minimum_file_size:1};let db=vec![entry(1,&root,ZERO_HASH,[1;32],10),entry(2,&root,ZERO_HASH,[2;32],20)];assert_eq!(build_dedupe_map(&Output,&args,&db).len(),2);}
#[test]fn already_shared_extent_hashes_are_filtered(){let root:PathRef=Rc::new("/photos".into());let args=Arguments{root_paths:vec![root.clone()],minimum_file_size:1};let db=vec![entry(1,&root,[7;32],[1;32],10),entry(2,&root,[7;32],[1;32],10)];assert!(build_dedupe_map(&Output,&args,&db).is_empty());}
#[test]fn different_content_hashes_remain_separate(){let root:PathRef=Rc::new("/photos".into());let args=Arguments{root_paths:vec![root.clone()],minimum_file_size:1};let db=vec![entry(1,&root,[7;32],[1;32],10),entry(2,&root,[8;32],[2;32],10)];assert!(build_dedupe_map(&Output,&args,&db).is_empty());}

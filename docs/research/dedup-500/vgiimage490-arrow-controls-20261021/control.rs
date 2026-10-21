#[path="../src/arrow_io.rs"] mod arrow_io;
#[path="../src/imaging.rs"] mod imaging;
#[path="../src/meta.rs"] mod meta;
#[path="../src/scalar/transform.rs"] mod transform;
use std::sync::Arc;
use arrow_array::{ArrayRef,StringArray};
use vgi::{ScalarFunction,BindParams};
use vgi::arguments::Arguments;
#[test]
fn review_bad_row_errors_whole_batch(){
 let good=arrow_io::test_support::make_png(4,4);
 for rows in [vec![Some(good.as_slice()),Some(b"nope".as_slice())],vec![Some(b"nope".as_slice()),Some(good.as_slice())]] {
 assert!(arrow_io::test_support::run_scalar(&transform::Thumbnail,&rows,Arguments::default()).is_err());
 }
 println!("Good/bad and bad/good batches both return error, no partial batch");
}
#[test]
fn review_direct_bind_wrong_width_defaults(){
 let args:Vec<ArrayRef>=["blob","not-an-integer","also-not-integer","png"].into_iter().map(|s|Arc::new(StringArray::from(vec![s])) as ArrayRef).collect();
 let args=Arguments::parse(&Arguments::serialize_positional(&args).unwrap()).unwrap();
 assert!(args.const_i64(1).is_none());
 let bind=BindParams{arguments:args.clone(),..Default::default()};
 assert!(transform::ThumbnailFit.on_bind(&bind).is_ok());
 let png=arrow_io::test_support::make_png(256,256);
 let output=arrow_io::test_support::run_scalar(&transform::ThumbnailFit,&[Some(&png)],args).unwrap();
 use arrow_array::cast::AsArray;
 let info=imaging::image_info(output.as_binary::<i32>().value(0)).unwrap();
 assert_eq!((info.width,info.height),(128,128));
 println!("Direct bind/process with string dimensions accepts/defaults128; external SDK discovery constraints not exercised");
}

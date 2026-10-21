#[path="../../../../.dedup-work/btrfs137-20261021/src/database/recursive_path.rs"] mod paths;
use paths::RecursivePathDatabase;
#[test]fn ancestor_compares_equal_to_distinct_descendant(){
 let mut db=RecursivePathDatabase::new();let a=db.for_path("/photos").unwrap();let b=db.for_path("/photos/child").unwrap();
 assert_ne!(a,b);assert_eq!(a.cmp(&b),std::cmp::Ordering::Equal);assert_eq!(b.cmp(&a),std::cmp::Ordering::Equal);
 // Only the Ord/Eq inconsistency is established; collection effects require separate controls.
}
#[test]fn siblings_remain_distinct(){let mut db=RecursivePathDatabase::new();let a=db.for_path("/photos/a").unwrap();let b=db.for_path("/photos/b").unwrap();assert_ne!(a.cmp(&b),std::cmp::Ordering::Equal);}
#[test]fn relative_path_is_explicitly_refused(){assert!(RecursivePathDatabase::new().for_path("photos/a").is_none());}
#[test]fn absolute_roundtrip_and_interning(){let mut db=RecursivePathDatabase::new();let a=db.for_path("/photos/a").unwrap();let b=db.for_path("/photos/a").unwrap();assert_eq!(a.to_path(),std::path::Path::new("/photos/a"));assert!(std::rc::Rc::ptr_eq(&a,&b));}

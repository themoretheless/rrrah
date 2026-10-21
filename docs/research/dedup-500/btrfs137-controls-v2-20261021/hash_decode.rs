extern crate rustc_serialize;
use rustc_serialize::hex::ToHex;
#[path="../../../../.dedup-work/btrfs137-20261021/src/types.rs"] mod types;
#[path="../../../../.dedup-work/btrfs137-20261021/src/misc.rs"] mod misc;
#[test] fn last_hash_byte_is_replaced_by_byte21(){let bytes:Vec<u8>=(0..32).collect();let decoded=misc::decode_hash(&Some(bytes.to_hex()));assert_eq!(&decoded[..31],&bytes[..31]);assert_eq!(decoded[31],21);assert_ne!(decoded[31],bytes[31]);}
#[test] fn distinct_final_digest_bytes_decode_identically(){let a=vec![1u8;32];let mut b=a.clone();b[31]=2;assert_eq!(misc::decode_hash(&Some(a.to_hex())),misc::decode_hash(&Some(b.to_hex())));}
#[test] fn absent_hash_is_zero(){assert_eq!(misc::decode_hash(&None),[0;32]);}
#[test] fn short_hash_panics(){assert!(std::panic::catch_unwind(||misc::decode_hash(&Some("00".into()))).is_err());}

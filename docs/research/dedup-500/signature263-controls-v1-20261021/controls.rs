#[path="../../../../.dedup-work/signature263-20261021/src/hash.rs"] mod hash;
#[path="../../../../.dedup-work/signature263-20261021/src/generator/lookup3_signature.rs"] mod signature;
#[test]fn low_seed_bits_ignored_by_64bit_hash(){
 let a=signature::Lookup3Signature{seed:1};let b=signature::Lookup3Signature{seed:2};
 assert_eq!(a.generate_sign_64("sample"),b.generate_sign_64("sample"));
}
#[test]fn high_seed_bits_ignored_by_32bit_hash(){
 let a=signature::Lookup3Signature{seed:1};let b=signature::Lookup3Signature{seed:1+(1<<32)};
 assert_eq!(a.generate_sign("sample"),b.generate_sign("sample"));
}
#[test]fn arithmetic_profile_boundary(){
 let result=std::panic::catch_unwind(||hash::lookup3(&vec![u32::MAX],0,1,0));
 if cfg!(debug_assertions){assert!(result.is_err());}else{assert!(result.is_ok());}
}

extern crate alloc;
#[path = "evidence/n24q02m--modhash/03-bktree.rs"] mod original;
fn main(){ let mut t=original::BkTree::new();t.insert(0,9);t.insert(1,1);let r=t.search(0,1);assert_eq!(r.iter().map(|c|(c.id,c.distance)).collect::<Vec<_>>(),vec![(1,1),(9,0)]);println!("actual={:?}; promised distance/id order=[(9,0),(1,1)]",r); assert_eq!(t.len(),2);assert!(!t.is_empty());}

use {
    std::{
        collections::{btree_map::Entry, BTreeMap, HashSet},
        sync::Arc,
    },
    tokio::sync::Mutex,
};

#[async_trait::async_trait]
pub trait KafkaDedup: Clone {
    async fn allowed(&self, slot: u64, hash: [u8; 32]) -> bool;
}

#[derive(Debug, Default, Clone)]
pub struct KafkaDedupMemory {
    inner: Arc<Mutex<BTreeMap<u64, HashSet<[u8; 32]>>>>,
}

#[async_trait::async_trait]
impl KafkaDedup for KafkaDedupMemory {
    async fn allowed(&self, slot: u64, hash: [u8; 32]) -> bool {
        let mut map = self.inner.lock().await;

        if let Some(key_slot) = map.keys().next().cloned() {
            if slot < key_slot {
                return false;
            }
        }

        match map.entry(slot) {
            Entry::Vacant(entry) => {
                entry.insert(HashSet::new()).insert(hash);

                // remove old sets, keep ~30sec log
                while let Some(key_slot) = map.keys().next().cloned() {
                    if key_slot < slot - 75 {
                        map.remove(&key_slot);
                    } else {
                        break;
                    }
                }

                true
            }
            Entry::Occupied(entry) => entry.into_mut().insert(hash),
        }
    }
}

#[cfg(test)] mod owned_controls {use super::*;
#[cfg(debug_assertions)] #[tokio::test] #[should_panic(expected="attempt to subtract with overflow")] async fn low_initial_slot_panics_debug(){KafkaDedupMemory::default().allowed(0,[0;32]).await;}
#[tokio::test] async fn same_slot_hash_rejected_but_other_slot_allowed(){let d=KafkaDedupMemory::default();assert!(d.allowed(75,[1;32]).await);assert!(!d.allowed(75,[1;32]).await);assert!(d.allowed(76,[1;32]).await);}
#[tokio::test] async fn slot_window_exact_boundary(){let d=KafkaDedupMemory::default();assert!(d.allowed(75,[1;32]).await);assert!(d.allowed(150,[2;32]).await);assert!(d.inner.lock().await.contains_key(&75));assert!(d.allowed(151,[3;32]).await);assert!(!d.inner.lock().await.contains_key(&75));assert!(!d.allowed(75,[4;32]).await);}
#[tokio::test] async fn same_slot_keeps_all_distinct_hashes(){let d=KafkaDedupMemory::default();for i in 0u32..1000{let mut h=[0;32];h[..4].copy_from_slice(&i.to_le_bytes());assert!(d.allowed(100,h).await);}assert_eq!(d.inner.lock().await.get(&100).unwrap().len(),1000);}
#[tokio::test] async fn cloned_concurrent_same_pair_one_winner(){let d=KafkaDedupMemory::default();let mut jobs=Vec::new();for _ in 0..32{let c=d.clone();jobs.push(tokio::spawn(async move{c.allowed(100,[9;32]).await}));}let mut won=0;for j in jobs{won+=usize::from(j.await.unwrap());}assert_eq!(won,1);}
}

#[cfg(all(test,not(debug_assertions)))] mod owned_release_controls {use super::*;
#[tokio::test] async fn low_slot_wrap_removes_own_entry_allows_repeat(){let d=KafkaDedupMemory::default();assert!(d.allowed(0,[0;32]).await);assert!(d.inner.lock().await.is_empty());assert!(d.allowed(0,[0;32]).await);}
}

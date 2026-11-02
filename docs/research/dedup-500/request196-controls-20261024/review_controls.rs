use deduplicate::{Deduplicate,DeduplicateFuture,DeduplicateError};
use futures::{poll,FutureExt};
use std::{sync::{Arc,atomic::{AtomicUsize,Ordering}},panic::AssertUnwindSafe};
use tokio::sync::Notify;
#[tokio::test]
async fn concurrent_identical_keys_share_one_delegate(){
 let calls=Arc::new(AtomicUsize::new(0));let gate=Arc::new(Notify::new());
 let d=Deduplicate::new({let calls=calls.clone();let gate=gate.clone();move |_|->DeduplicateFuture<u64>{calls.fetch_add(1,Ordering::SeqCst);let gate=gate.clone();Box::pin(async move {gate.notified().await;Some(17)})}});
 let mut group=Box::pin(futures::future::join_all((0..16).map(|_|d.get(1u64))));assert!(poll!(group.as_mut()).is_pending());assert_eq!(calls.load(Ordering::SeqCst),1);gate.notify_one();let rows=group.await;assert!(rows.iter().all(|x|matches!(x,Ok(Some(17)))));assert_eq!(d.count(),1);
}
#[tokio::test]
async fn negative_none_is_not_cached(){
 let calls=Arc::new(AtomicUsize::new(0));let d=Deduplicate::new({let calls=calls.clone();move |_|->DeduplicateFuture<u64>{calls.fetch_add(1,Ordering::SeqCst);Box::pin(async {None})}});assert_eq!(d.get(1u64).await.unwrap(),None);assert_eq!(d.get(1).await.unwrap(),None);assert_eq!(calls.load(Ordering::SeqCst),2);assert_eq!(d.count(),0);
}
#[tokio::test]
async fn clearing_cache_does_not_invalidate_inflight_result(){
 let gate=Arc::new(Notify::new());let d=Deduplicate::new({let gate=gate.clone();move |_|->DeduplicateFuture<u64>{let gate=gate.clone();Box::pin(async move {gate.notified().await;Some(17)})}});let mut first=Box::pin(d.get(1u64));assert!(poll!(first.as_mut()).is_pending());d.clear();assert_eq!(d.request_count(),0);gate.notify_one();assert_eq!(first.await.unwrap(),Some(17));assert_eq!(d.count(),1);
}
#[tokio::test]
async fn cancelled_origin_leaves_one_failed_call_after_successful_follower(){
 let calls=Arc::new(AtomicUsize::new(0));let gate=Arc::new(Notify::new());let d=Deduplicate::new({let calls=calls.clone();let gate=gate.clone();move |_|->DeduplicateFuture<u64>{calls.fetch_add(1,Ordering::SeqCst);let gate=gate.clone();Box::pin(async move {gate.notified().await;Some(17)})}});let mut origin=Box::pin(d.get(1u64));assert!(poll!(origin.as_mut()).is_pending());drop(origin);let mut follower=Box::pin(d.get(1u64));assert!(poll!(follower.as_mut()).is_pending());gate.notify_one();assert_eq!(follower.await.unwrap(),Some(17));assert_eq!(d.count(),0);assert!(matches!(d.get(1).await,Err(DeduplicateError::Failed)));assert_eq!(calls.load(Ordering::SeqCst),1);gate.notify_one();assert_eq!(d.get(1).await.unwrap(),Some(17));assert_eq!(calls.load(Ordering::SeqCst),2);
}
#[tokio::test]
async fn synchronous_factory_panic_requires_cleanup_retry(){
 let calls=Arc::new(AtomicUsize::new(0));let d=Deduplicate::new({let calls=calls.clone();move |_|->DeduplicateFuture<u64>{if calls.fetch_add(1,Ordering::SeqCst)==0{panic!("owned injected factory panic")}Box::pin(async {Some(17)})}});assert!(AssertUnwindSafe(d.get(1u64)).catch_unwind().await.is_err());assert!(matches!(d.get(1).await,Err(DeduplicateError::Failed)));assert_eq!(d.get(1).await.unwrap(),Some(17));
}

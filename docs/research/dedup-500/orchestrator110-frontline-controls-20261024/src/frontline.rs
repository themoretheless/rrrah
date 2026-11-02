use async_trait::async_trait;
use std::{collections::HashMap,sync::{Arc,atomic::{AtomicBool,AtomicUsize,Ordering}},time::Duration};
use crate::{OrchestratorError,enhanced::{dedup_key,DeduplicationResult,Deduplicator}};
#[async_trait] trait ModelWorker: Send+Sync {async fn infer(&self,prompt:&str)->Result<Vec<String>,OrchestratorError>;}
struct Frontline {
    backend: Arc<dyn ModelWorker>,
    dedup: Deduplicator,
    backend_calls: AtomicUsize,
    outage: AtomicBool,
}

#[async_trait]
impl ModelWorker for Frontline {
    async fn infer(&self, prompt: &str) -> Result<Vec<String>, OrchestratorError> {
        if self.outage.load(Ordering::SeqCst) {
            return Err(OrchestratorError::Inference(
                "503 Service Unavailable (simulated outage)".into(),
            ));
        }

        let key = dedup_key(prompt, &HashMap::new(), None);
        match self.dedup.check_and_register(&key).await {
            DeduplicationResult::Cached(json) => decode(&json),
            DeduplicationResult::InProgress => match self.dedup.wait_for_result(&key).await {
                Some(json) => decode(&json),
                None => self.backend.infer(prompt).await,
            },
            DeduplicationResult::New(token) => {
                self.backend_calls.fetch_add(1, Ordering::SeqCst);
                match self.backend.infer(prompt).await {
                    Ok(tokens) => {
                        let json = serde_json::to_string(&tokens).unwrap_or_default();
                        self.dedup.complete(token, json).await;
                        Ok(tokens)
                    }
                    Err(e) => {
                        self.dedup.fail(token).await;
                        Err(e)
                    }
                }
            }
        }
    }
}

fn decode(json: &str) -> Result<Vec<String>, OrchestratorError> {
    serde_json::from_str(json).map_err(|e| OrchestratorError::Other(e.to_string()))
}


#[cfg(test)] mod controls {
use super::*;
struct OwnedBackend{calls:AtomicUsize}
#[async_trait] impl ModelWorker for OwnedBackend{async fn infer(&self,_:&str)->Result<Vec<String>,OrchestratorError>{self.calls.fetch_add(1,Ordering::SeqCst);Ok(vec!["owned answer".into()])}}
fn front()->(Frontline,Arc<OwnedBackend>){let backend=Arc::new(OwnedBackend{calls:AtomicUsize::new(0)});(Frontline{backend:backend.clone(),dedup:Deduplicator::new(Duration::from_secs(60)),backend_calls:AtomicUsize::new(0),outage:AtomicBool::new(false)},backend)}
#[tokio::test] async fn cached_repeat_reuses_one_call(){let(f,b)=front();assert_eq!(f.infer("same").await.unwrap(),vec!["owned answer"]);assert_eq!(f.infer("same").await.unwrap(),vec!["owned answer"]);assert_eq!(b.calls.load(Ordering::SeqCst),1);assert_eq!(f.backend_calls.load(Ordering::SeqCst),1);}
#[tokio::test] async fn waiting_failure_is_json_error_without_backend_retry(){let(f,b)=front();let key=dedup_key("same",&HashMap::new(),None);let token=match f.dedup.check_and_register(&key).await{DeduplicationResult::New(t)=>t,_=>panic!("fresh")};let mut waiting=Box::pin(f.infer("same"));let waker=std::task::Waker::noop();let mut cx=std::task::Context::from_waker(waker);assert!(std::future::Future::poll(waiting.as_mut(),&mut cx).is_pending());f.dedup.fail(token).await;let result=tokio::time::timeout(Duration::from_secs(2),waiting).await.unwrap();match result{Err(OrchestratorError::Other(s))=>assert!(s.contains("expected value"),"{s}"),other=>panic!("unexpected {other:?}")};assert_eq!(b.calls.load(Ordering::SeqCst),0);assert_eq!(f.backend_calls.load(Ordering::SeqCst),0);}
}

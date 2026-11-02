pub mod embedding;pub mod enhanced;
#[derive(Debug)] pub enum OrchestratorError {Other(String),Inference(String)}
impl std::fmt::Display for OrchestratorError{fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{write!(f,"{:?}",self)}}
pub mod metrics{pub fn inc_dedup_hit(){} pub fn inc_dedup_waiter_unblocked(){}}

pub mod simhash;

pub mod pipeline;

#[cfg(test)] mod frontline;

#[derive(Clone, Copy, Debug)]
pub struct NativeBankDirectionSummary {
    pub support: Option<bool>,
    pub model_attempted: bool,
    /// Bounded candidate diagnostics survive direction compaction. None means
    /// no model-bank trace was produced; zero counts are not a negative verdict.
    pub model_diagnostics: Option<crate::model_banks::ModelBankDiagnostics>,
}

/// Both directions observed under one source/dependency/weights lifetime.
/// This is local visual evidence, not proof of whole-file identity or uniqueness.
#[derive(Clone, Copy, Debug)]
pub struct NativeBankConsensusEvidence {
    pub forward: NativeBankDirectionSummary,
    pub reverse: NativeBankDirectionSummary,
}
impl NativeBankConsensusEvidence {
    /// Missing directional support stays unknown; it is never a rejection.
    pub fn supported(&self) -> Option<bool> {
        if self.forward.support == Some(true) && self.reverse.support == Some(true) {
            Some(true)
        } else {
            None
        }
    }
}


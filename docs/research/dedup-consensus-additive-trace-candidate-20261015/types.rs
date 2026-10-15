#[derive(Debug, Clone, Copy)]
pub enum InsufficientAnchorStage {
    Native,
    Model {
        model_digest: [u8; 32],
    },
    ModelTiles {
        attempts: usize,
        insufficient_attempts: usize,
        model_digest: [u8; 32],
    },
}

/// Small audit record for the unsupported first stage. Its managed candidate
/// buffers are released before the second stage is admitted.
#[derive(Debug, Clone, Copy)]
pub enum SearchStageMiss {
    InsufficientAnchorGeometry(InsufficientAnchorStage),
    Native {
        recipes: u32,
        candidates: usize,
    },
    Model {
        model_digest: [u8; 32],
        raw_proposals: usize,
        selected_points: usize,
    },
    /// Counts refer to the last unsupported region; `attempts` counts the whole
    /// exhausted model recipe. Earlier attempt buffers are no longer retained.
    ModelTiles {
        attempts: usize,
        model_digest: [u8; 32],
        raw_proposals: usize,
        selected_points: usize,
    },
}
#[derive(Clone, Copy, Debug)]
pub struct NativeBankDirectionSummary {
    pub support: Option<bool>,
    pub model_attempted: bool,
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

/// Compact stage attribution; counters describe bounded candidates, never a negative verdict.
#[derive(Clone, Copy, Debug)]
pub struct NativeBankRecoveryTrace {
    pub support: Option<bool>,
    pub model_attempted: bool,
    pub native_miss: Option<SearchStageMiss>,
    pub opposite_insufficient: Option<bool>,
    pub model_attempts: Option<usize>,
    pub model_diagnostics: Option<crate::model_banks::ModelBankDiagnostics>,
}
/// Both compact traces share the existing operation-atomic observation lifetime.
#[derive(Clone, Copy, Debug)]
pub struct NativeBankConsensusTraceEvidence {
    pub forward: NativeBankRecoveryTrace,
    pub reverse: NativeBankRecoveryTrace,
}
impl NativeBankConsensusTraceEvidence {
    pub fn summary(&self) -> NativeBankConsensusEvidence {
        let summarize = |trace: NativeBankRecoveryTrace| NativeBankDirectionSummary {
            support: trace.support,
            model_attempted: trace.model_attempted,
        };
        NativeBankConsensusEvidence {
            forward: summarize(self.forward),
            reverse: summarize(self.reverse),
        }
    }
    pub fn supported(&self) -> Option<bool> {
        self.summary().supported()
    }
}


#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ModelBankDiagnostics {
    pub geometry_candidates: u64,
    pub pixel_geometry_refusals: u64,
    pub pixel_rejections: u64,
}

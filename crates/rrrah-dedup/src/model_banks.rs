//! Guarded whole/quadrant point-bank aggregation and original-file confirmation.
use crate::{
    anchor_rank_file::{compare_anchor_rank_files, AnchorRankFileError},
    exact::ContentSnapshot,
    geometry::GeometryError,
    joint_witnesses::{JointBankPolicy, search_joint_bank_candidate, search_joint_bank_candidate_with},
    local_rank::LocalRankError,
    model_compare::{extract_model_region_points_files_with, validate_model_comparison_policy, ModelComparisonError, ModelComparisonEvidence, ModelComparisonPolicy, ModelPointBank, Provider},
    model_input_file::prepare_file,
    model_tiles::{ModelTileError, ModelTileWorkLimits, regions, owners_cancelled, validate_model_tile_work},
};
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use std::{cell::Cell,path::Path};

#[derive(Debug, Clone, Copy)]
pub struct ModelBankSearchPolicy {
    pub comparison: ModelComparisonPolicy,
    pub banks: JointBankPolicy,
    /// Includes 25 crop pairs and the two full-image preparations for guards.
    pub work: ModelTileWorkLimits,
}
/// Search diagnostics describe only the bounded candidates examined. A rejected
/// candidate does not prove that the original pair has no visual relationship.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ModelBankDiagnostics {
    pub geometry_candidates: u64,
    pub pixel_geometry_refusals: u64,
    pub pixel_rejections: u64,
}
#[derive(Debug, Clone, Copy)]
pub struct ModelBankSearchEvidence {
    pub attempts: usize,
    pub hypothesis_bank: Option<usize>,
    pub pixel_window_radius: Option<u32>,
    pub pixel_rank_radius: Option<u32>,
    pub pixel_attempts: u64,
    pub diagnostics: ModelBankDiagnostics,
    pub comparison: ModelComparisonEvidence,
}
/// Explicit pixel context, independent of model extraction and geometry.
#[derive(Debug, Clone, Copy)]
pub struct ModelBankPixelRecipe {
    pub window_radius: u32,
    pub rank_radius: u32,
}
impl ModelBankSearchEvidence {
    pub fn supported(&self)->bool {self.comparison.supported()}
}

/// Validate conservative 25-window work before source/model I/O.
pub(crate) fn validate_model_bank_coverage_policy(policy:ModelBankSearchPolicy,radii:&[u32],budget:&MemoryBudget)->Result<(),ModelTileError> {
    if radii.is_empty() || radii.len()>8 {return Err(ModelComparisonError::Policy.into());}
    let p=policy.comparison;let b=policy.banks;
    validate_model_comparison_policy(p)?;
    validate_model_tile_work(p,policy.work)?;
    // Validate all constituent bank policy scalars without reading any files.
    search_joint_bank_candidate(&[],(1,1),(1,1),b,budget,||false).map_err(ModelComparisonError::Geometry)?;
    if b.minimum_witnesses<p.anchors.local.minimum_witnesses || b.minimum_witnesses<p.geometry.min_inliers
        || b.hypothesis.target.max_points<p.proposals.max_proposals
        || b.hypothesis.target.tolerance!=p.geometry.tolerance || b.witnesses.source_tolerance!=p.anchors.local.source_tolerance
        || b.witnesses.target_tolerance!=p.anchors.local.target_tolerance
        || b.witnesses.minimum_separation!=p.anchors.local.minimum_point_separation {return Err(ModelComparisonError::Policy.into());}
    let n=u64::try_from(p.proposals.max_proposals).ok().and_then(|v|v.checked_mul(25)).ok_or(ModelTileError::Work)?;
    let pad=p.anchors.local.rank.radius.checked_add(p.anchors.local.filter_radius).ok_or(ModelTileError::Work)?;
    for radius in radii {
        let size=radius.checked_mul(2).and_then(|v|v.checked_add(1)).ok_or(ModelTileError::Work)?;
        let extent=u64::from(size).checked_add(2*u64::from(pad)).ok_or(ModelTileError::Work)?;
        n.checked_mul(2).and_then(|v|v.checked_mul(extent)).and_then(|v|v.checked_mul(extent)).and_then(|v|v.checked_mul(5)).ok_or(ModelTileError::Work)?;
    }
    let pixel_calls=25u64.checked_mul(radii.len() as u64).ok_or(ModelTileError::Work)?;
    for (per_call,limit) in [
        (p.anchors.local.maximum_point_checks,policy.work.point_checks),
        (p.anchors.maximum_selection_checks,policy.work.selection_checks),
        (p.anchors.local.rank.maximum_sites,policy.work.rank_sites),
        (p.anchors.local.rank.maximum_pixel_reads,policy.work.pixel_reads),
    ] {if per_call.checked_mul(pixel_calls).ok_or(ModelTileError::Work)?>limit {return Err(ModelTileError::Work);}}
    let fit=u64::try_from(b.hypothesis.target.max_points).ok().and_then(|v|v.checked_mul(2))
        .and_then(|v|b.sampling.trials.checked_add(1).and_then(|t|v.checked_mul(t))).ok_or(ModelTileError::Work)?;
    let collect=n.checked_mul(2).ok_or(ModelTileError::Work)?;
    let separation=n.checked_mul(n.saturating_sub(1)).and_then(|v|v.checked_div(2)).ok_or(ModelTileError::Work)?;
    let total=fit.checked_add(collect).and_then(|v|v.checked_mul(25)).ok_or(ModelTileError::Work)?;
    let total_separation=separation.checked_mul(25).ok_or(ModelTileError::Work)?;
    let hypotheses=policy.banks.sampling.trials.checked_mul(25).ok_or(ModelTileError::Work)?;
    let reads=p.input.max_pixel_reads.checked_mul(52).ok_or(ModelTileError::Work)?;
    if b.maximum_banks<25 || u64::try_from(b.witnesses.maximum_points).map_err(|_|ModelTileError::Work)?<n
        || b.hypothesis.maximum_projection_checks<fit || b.witnesses.maximum_projection_checks<collect
        || b.witnesses.maximum_separation_checks<separation || b.maximum_total_projection_checks<total
        || b.maximum_total_separation_checks<total_separation || policy.work.model_input_reads<reads || policy.work.geometry_hypotheses<hypotheses {return Err(ModelTileError::Work);}
    Ok(())
}

/// Extract all whole/quadrant Cartesian banks, generate a joint candidate and
/// confirm original pixels in one operation. Original file/dependency and weights
/// observations remain live across every extraction and fresh pixel comparison.
/// No bank/model/geometry miss proves unrelatedness. Every eligible bank candidate is pixel-tested before ranking;
/// absent pixel support remains inconclusive. Model tensors,
/// fitter scratch and the bounded 25-entry reference vector are outside managed
/// payload accounting; banks, inputs, flattened pool and witnesses are managed.
/// No partial result survives late cancellation, source mutation or admission error.
#[allow(clippy::too_many_arguments)]
pub fn search_model_banks_files_with(
    left:&DecodeRequest,right:&DecodeRequest,weights:&Path,policy:ModelBankSearchPolicy,
    budget:&MemoryBudget,provider:&mut Provider<'_>,cancel:impl Fn()->bool,
)->Result<ModelBankSearchEvidence,ModelTileError> {
    search_model_banks_coverage_files_with(left,right,weights,policy,&[policy.comparison.anchors.window_radius],budget,provider,cancel)
}

/// Try up to eight caller-ordered pixel window radii per candidate, stopping at
/// its first original-pixel support. All radius work/overflow is admitted before
/// I/O. Inference and geometry are shared; thresholds and source guards remain
/// unchanged. Typed insufficient anchor geometry tries the next radius; other
/// errors are fatal. A larger radius need not provide more disjoint anchors.
#[allow(clippy::too_many_arguments)]
pub fn search_model_banks_coverage_files_with(
    left:&DecodeRequest,right:&DecodeRequest,weights:&Path,policy:ModelBankSearchPolicy,
    radii:&[u32],budget:&MemoryBudget,provider:&mut Provider<'_>,cancel:impl Fn()->bool,
)->Result<ModelBankSearchEvidence,ModelTileError> {
    if radii.is_empty() || radii.len()>8 {
        if cancel(){return Err(ModelTileError::Cancelled);}
        return Err(ModelComparisonError::Policy.into());
    }
    let mut recipes=[ModelBankPixelRecipe{window_radius:0,rank_radius:policy.comparison.anchors.local.rank.radius};8];
    for (recipe,radius) in recipes.iter_mut().zip(radii) {recipe.window_radius=*radius;}
    search_model_banks_pixel_coverage_files_with(left,right,weights,policy,&recipes[..radii.len()],budget,provider,cancel)
}

/// Try up to eight explicit pixel contexts per eligible geometric candidate.
/// Extraction is shared across contexts; agreement, contrast and pair thresholds
/// remain caller supplied and unchanged. Every context's worst-case work is
/// admitted before I/O. A miss remains inconclusive. No automatic parameter
/// selection or general precision guarantee is implied by this entry point.
#[allow(clippy::too_many_arguments)]
pub fn search_model_banks_pixel_coverage_files_with(
    left:&DecodeRequest,right:&DecodeRequest,weights:&Path,policy:ModelBankSearchPolicy,
    recipes:&[ModelBankPixelRecipe],budget:&MemoryBudget,provider:&mut Provider<'_>,cancel:impl Fn()->bool,
)->Result<ModelBankSearchEvidence,ModelTileError> {
    search_model_banks_pixel_selected(left,right,weights,policy,recipes,None,budget,provider,cancel)
        .map(|e|e.search)
}

/// Explicit overlapping-source search with calibrated original-pixel evidence.
/// Exposure calibration can accept unrelated smooth regions; caller policies
/// require independent precision qualification. Defaults never select this path.
#[derive(Debug, Clone, Copy)]
pub struct ExposureModelBankSearchEvidence {
    pub search: ModelBankSearchEvidence,
    /// Present only for the selected pixel-supported candidate, in input order.
    pub exposures: Option<[crate::exposure::RankExposure;2]>,
}

/// Nine half-image source windows at0/quarter/half offsets against the full
/// reference. Model extraction is shared across all pixel recipes. Whole-source,
/// decoder-dependency and weight guards span the complete operation. Existing
/// conservative25-bank admission applies, including25*recipes*6*decode.max_pixels
/// calibration reads before I/O. This is an explicit recovery recipe, not an
/// automatic replacement of ordinary searches or a whole-file identity verdict.
#[allow(clippy::too_many_arguments)]
pub fn search_model_banks_overlap_exposure_files_with(
    left:&DecodeRequest,right:&DecodeRequest,weights:&Path,policy:ModelBankSearchPolicy,
    recipes:&[ModelBankPixelRecipe],maximum_calibration_pixel_reads:u64,
    budget:&MemoryBudget,provider:&mut Provider<'_>,cancel:impl Fn()->bool,
)->Result<ExposureModelBankSearchEvidence,ModelTileError> {
    search_model_banks_pixel_selected(left,right,weights,policy,recipes,
        Some(maximum_calibration_pixel_reads),budget,provider,cancel)
}

/// Pure policy/work validation shared by the explicit two-stage recovery.
pub(crate) fn validate_model_bank_pixel_policy(
    policy:ModelBankSearchPolicy,recipes:&[ModelBankPixelRecipe],
    maximum_calibration_pixel_reads:Option<u64>,budget:&MemoryBudget,
)->Result<(),ModelTileError> {
    if recipes.is_empty() || recipes.len()>8 || recipes.iter().any(|r|r.rank_radius==0) {return Err(ModelComparisonError::Policy.into());}
    let mut admission=policy;
    admission.comparison.anchors.local.rank.radius=recipes.iter().map(|r|r.rank_radius).max().ok_or(ModelComparisonError::Policy)?;
    let mut radii=[0u32;8];
    for (radius,recipe) in radii.iter_mut().zip(recipes) {*radius=recipe.window_radius;}
    validate_model_bank_coverage_policy(admission,&radii[..recipes.len()],budget)?;
    if let Some(limit)=maximum_calibration_pixel_reads {
        let required=policy.comparison.decode.max_pixels.checked_mul(6)
            .and_then(|n|n.checked_mul(25)).and_then(|n|n.checked_mul(recipes.len() as u64))
            .ok_or(ModelTileError::Work)?;
        if required>limit {return Err(ModelTileError::Work);}
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn search_model_banks_pixel_selected(
    left:&DecodeRequest,right:&DecodeRequest,weights:&Path,policy:ModelBankSearchPolicy,
    recipes:&[ModelBankPixelRecipe],maximum_calibration_pixel_reads:Option<u64>,budget:&MemoryBudget,provider:&mut Provider<'_>,cancel:impl Fn()->bool,
)->Result<ExposureModelBankSearchEvidence,ModelTileError> {
    let latch=Cell::new(false);
    let stopped=||{let v=owners_cancelled(left,right,&latch)||cancel();latch.set(v);v};
    if stopped(){return Err(ModelTileError::Cancelled);}
    validate_model_bank_pixel_policy(policy,recipes,maximum_calibration_pixel_reads,budget)?;
    let _slots_credit=budget.try_reserve(25 * std::mem::size_of::<Option<ModelPointBank>>() as u64)?;
    let mut slots=Vec::new();
    slots.try_reserve_exact(25).map_err(|_|ModelComparisonError::Geometry(GeometryError::Budget))?;
    if slots.capacity()>25 {return Err(ModelTileError::Work);}
    slots.resize_with(25,||None::<ModelPointBank>);
    let result=(|| {
        let p=policy.comparison;
        let model=ContentSnapshot::read(weights,p.max_model_bytes,&stopped)?;
        let l=prepare_file(left,None,p.decode,p.input,budget,&stopped).map_err(ModelComparisonError::Left)?;
        let r=prepare_file(right,None,p.decode,p.input,budget,&stopped).map_err(ModelComparisonError::Right)?;
        let verify=|checkpoint:&dyn Fn()->bool|->Result<(),ModelTileError>{
            l.verify_source(checkpoint).map_err(ModelComparisonError::Left)?;
            r.verify_source(checkpoint).map_err(ModelComparisonError::Right)?;
            model.verify(checkpoint)?;Ok(())
        };
        verify(&stopped)?;
        let ld=l.original_dimensions();let rd=r.original_dimensions();
        let mut pairs=[([0u32;4],[0u32;4]);25];let mut pair_count=0;
        if maximum_calibration_pixel_reads.is_some() {
            let width=ld[0]/2;let height=ld[1]/2;
            if width==0 || height==0 {return Err(ModelComparisonError::Policy.into());}
            for y in [0,ld[1]/4,ld[1]/2] {for x in [0,ld[0]/4,ld[0]/2] {
                pairs[pair_count]=([x,y,width,height],[0,0,rd[0],rd[1]]);pair_count+=1;
            }}
        } else {
            let (lr,ln)=regions(ld);let (rr,rn)=regions(rd);
            for a in &lr[..ln] {for b in &rr[..rn] {pairs[pair_count]=(*a,*b);pair_count+=1;}}
        }
        let mut count=0;let mut raw=0usize;
        for (a,b) in &pairs[..pair_count] {
            if stopped(){return Err(ModelTileError::Cancelled);}
            verify(&stopped)?;
            let bank=extract_model_region_points_files_with(left,right,weights,*a,*b,p,budget,&mut *provider,&stopped)?;
            verify(&stopped)?;
            if bank.original_dimensions!=[ld,rd] || bank.model_digest!=model.digest(){return Err(ModelComparisonError::Policy.into());}
            raw=raw.checked_add(bank.raw_proposals).ok_or(ModelTileError::Work)?;
            slots[count]=Some(bank);count+=1;
        }
        let mut refs=Vec::new();refs.try_reserve_exact(count).map_err(|_|ModelComparisonError::Geometry(GeometryError::Budget))?;
        for slot in slots[..count].iter(){refs.push(&slot.as_ref().ok_or(ModelComparisonError::Policy)?.points[..]);}
        let selected_points=refs.iter().try_fold(0usize,|n,p|n.checked_add(p.len())).ok_or(ModelTileError::Work)?;
        let mut confirmed_pixels=None;
        let mut confirmed_exposures=None;
        let mut confirmed_count=0;
        let mut confirmed_radius=None;
        let mut confirmed_rank_radius=None;
        let mut pixel_attempts=0u64;
        let mut diagnostics=ModelBankDiagnostics::default();
        let candidate=search_joint_bank_candidate_with(&refs,(ld[0],ld[1]),(rd[0],rd[1]),policy.banks,budget,&stopped,
            |candidate|->Result<bool,ModelComparisonError> {
                diagnostics.geometry_candidates+=1;
                for recipe in recipes {
                    if stopped(){return Err(ModelComparisonError::Cancelled);}
                    pixel_attempts+=1;
                    let mut anchors=p.anchors;anchors.window_radius=recipe.window_radius;anchors.local.rank.radius=recipe.rank_radius;
                    let result=if maximum_calibration_pixel_reads.is_some() {
                        crate::anchor_rank_file::compare_exposure_anchor_rank_files(left,right,candidate.transform,
                            &candidate.witnesses,p.decode,anchors,p.decode.max_pixels.checked_mul(6)
                                .ok_or(ModelComparisonError::Policy)?,budget,&stopped)
                            .map(|e|(e.anchors,Some(e.exposures)))
                    } else {
                        compare_anchor_rank_files(left,right,candidate.transform,&candidate.witnesses,p.decode,
                            anchors,budget,&stopped).map(|e|(e,None))
                    };
                    let (pixels,exposures)=match result {
                        Ok(e)=>e,
                        Err(AnchorRankFileError::Rank(LocalRankError::InsufficientGeometry))=>{diagnostics.pixel_geometry_refusals+=1;continue;},
                        Err(e)=>return Err(ModelComparisonError::Pixels(e)),
                    };
                    if pixels.supported {
                        if candidate.witnesses.len()>confirmed_count {
                            confirmed_count=candidate.witnesses.len();confirmed_pixels=Some(pixels);confirmed_exposures=exposures;confirmed_radius=Some(recipe.window_radius);confirmed_rank_radius=Some(recipe.rank_radius);
                        }
                        return Ok(true);
                    }
                    diagnostics.pixel_rejections+=1;
                }
                Ok(false)
            })?;
        let mut comparison=ModelComparisonEvidence{original_dimensions:[ld,rd],model_digest:model.digest(),raw_proposals:raw,selected_points,transform:None,geometry_inliers:0,hypotheses:0,pixels:None};
        let mut hypothesis_bank=None;
        if let Some(candidate)=candidate {
            comparison.transform=Some(candidate.transform);comparison.geometry_inliers=candidate.witnesses.len();comparison.hypotheses=policy.banks.sampling.trials;hypothesis_bank=Some(candidate.hypothesis_bank);
            comparison.pixels=confirmed_pixels;
        }
        verify(&stopped)?;
        if stopped(){return Err(ModelTileError::Cancelled);}
        let owners=||owners_cancelled(left,right,&latch);
        verify(&owners)?;
        if owners(){return Err(ModelTileError::Cancelled);}
        Ok(ExposureModelBankSearchEvidence{search:ModelBankSearchEvidence{attempts:count,hypothesis_bank,pixel_window_radius:confirmed_radius,pixel_rank_radius:confirmed_rank_radius,pixel_attempts,diagnostics,comparison},exposures:confirmed_exposures})
    })();
    // Cancellation has precedence even over a provider/source error.
    if result.is_err() && stopped(){return Err(ModelTileError::Cancelled);}
    if owners_cancelled(left,right,&latch){Err(ModelTileError::Cancelled)}else{result}
}

/// Explicit ordinary search followed only on an inconclusive result by the
/// overlapping calibrated recipe. Both complete work maxima are admitted even
/// when ordinary evidence allows skipping recovery. No default uses this policy.
#[derive(Debug, Clone, Copy)]
pub struct ModelBankExposureRecoveryPolicy {
    pub primary: ModelBankSearchPolicy,
    pub recovery: ModelBankSearchPolicy,
    pub work: ModelTileWorkLimits,
    pub maximum_calibration_pixel_reads: u64,
}
impl ModelBankExposureRecoveryPolicy {
    /// Checked sum of the caller-supplied stage maxima. Constituent scalar and
    /// recipe validation still occurs at search preflight before any I/O.
    pub fn for_stages(primary:ModelBankSearchPolicy,recovery:ModelBankSearchPolicy,
        maximum_calibration_pixel_reads:u64)->Result<Self,ModelTileError> {
        let a=primary.work;let b=recovery.work;
        let add=|x:u64,y:u64|x.checked_add(y).ok_or(ModelTileError::Work);
        Ok(Self{primary,recovery,maximum_calibration_pixel_reads,work:ModelTileWorkLimits{
            model_input_reads:add(a.model_input_reads,b.model_input_reads)?,
            proposal_ranking_checks:add(a.proposal_ranking_checks,b.proposal_ranking_checks)?,
            proposal_separation_checks:add(a.proposal_separation_checks,b.proposal_separation_checks)?,
            geometry_hypotheses:add(a.geometry_hypotheses,b.geometry_hypotheses)?,
            point_checks:add(a.point_checks,b.point_checks)?,
            selection_checks:add(a.selection_checks,b.selection_checks)?,
            rank_sites:add(a.rank_sites,b.rank_sites)?,pixel_reads:add(a.pixel_reads,b.pixel_reads)?,
        }})
    }
}
#[derive(Debug, Clone, Copy)]
pub struct ModelBankExposureRecoveryEvidence {
    pub primary: ModelBankSearchEvidence,
    /// Absent when primary evidence was already supported.
    pub recovery: Option<ExposureModelBankSearchEvidence>,
}
impl ModelBankExposureRecoveryEvidence {
    pub fn supported(&self)->bool {
        self.primary.supported() || self.recovery.is_some_and(|e|e.search.supported())
    }
}

pub(crate) fn validate_exposure_recovery_policy(
    policy:ModelBankExposureRecoveryPolicy,
    primary_recipes:&[ModelBankPixelRecipe],recovery_recipes:&[ModelBankPixelRecipe],
    budget:&MemoryBudget,
)->Result<(),ModelTileError> {
    validate_model_bank_pixel_policy(policy.primary,primary_recipes,None,budget)?;
    validate_model_bank_pixel_policy(policy.recovery,recovery_recipes,
        Some(policy.maximum_calibration_pixel_reads),budget)?;
    let required=ModelBankExposureRecoveryPolicy::for_stages(policy.primary,policy.recovery,
        policy.maximum_calibration_pixel_reads)?.work;
    let cap=policy.work;
    for (needed,limit) in [
        (required.model_input_reads,cap.model_input_reads),
        (required.proposal_ranking_checks,cap.proposal_ranking_checks),
        (required.proposal_separation_checks,cap.proposal_separation_checks),
        (required.geometry_hypotheses,cap.geometry_hypotheses),
        (required.point_checks,cap.point_checks),(required.selection_checks,cap.selection_checks),
        (required.rank_sites,cap.rank_sites),(required.pixel_reads,cap.pixel_reads),
    ] {if needed>limit {return Err(ModelTileError::Work);}}
    Ok(())
}

/// Preserve an ordinary supported result, otherwise try the explicit calibrated
/// overlap recipe with the same provider. Outer encoded-file, decoder-dependency
/// and weight observations span both operations, including skipped recovery.
/// Source byte guards use the larger stage file/model caps; they require no extra
/// model-input preparation. All stage limits and their cumulative sum are checked
/// before file/provider I/O. Errors remain fatal; only inconclusive evidence
/// advances. A result is local visual support, not file identity. Provider caches
/// and tensors/geometry scratch remain outside managed payload admission.
#[allow(clippy::too_many_arguments)]
pub fn search_model_banks_exposure_recovery_files_with(
    left:&DecodeRequest,right:&DecodeRequest,weights:&Path,
    policy:ModelBankExposureRecoveryPolicy,
    primary_recipes:&[ModelBankPixelRecipe],recovery_recipes:&[ModelBankPixelRecipe],
    budget:&MemoryBudget,provider:&mut Provider<'_>,cancel:impl Fn()->bool,
)->Result<ModelBankExposureRecoveryEvidence,ModelTileError> {
    let latch=Cell::new(false);
    let stopped=||{let v=owners_cancelled(left,right,&latch)||cancel();latch.set(v);v};
    if stopped(){return Err(ModelTileError::Cancelled);}
    validate_exposure_recovery_policy(policy,primary_recipes,recovery_recipes,budget)?;
    let result=(|| {
        let file_cap=policy.primary.comparison.decode.max_file_bytes
            .max(policy.recovery.comparison.decode.max_file_bytes);
        let weight_cap=policy.primary.comparison.max_model_bytes
            .max(policy.recovery.comparison.max_model_bytes);
        let first=crate::decode::DecodeSourceSnapshot::read(&left.path,file_cap,&stopped)?;
        let second=crate::decode::DecodeSourceSnapshot::read(&right.path,file_cap,&stopped)?;
        let model=ContentSnapshot::read(weights,weight_cap,&stopped)?;
        let verify=|checkpoint:&dyn Fn()->bool|->Result<(),ModelTileError>{
            first.verify(checkpoint)?;second.verify(checkpoint)?;model.verify(checkpoint)?;Ok(())
        };
        verify(&stopped)?;
        let primary=search_model_banks_pixel_coverage_files_with(left,right,weights,
            policy.primary,primary_recipes,budget,&mut *provider,&stopped)?;
        verify(&stopped)?;
        let recovery=if primary.supported(){None}else{
            Some(search_model_banks_overlap_exposure_files_with(left,right,weights,
                policy.recovery,recovery_recipes,policy.maximum_calibration_pixel_reads,
                budget,&mut *provider,&stopped)?)
        };
        verify(&stopped)?;
        if stopped(){return Err(ModelTileError::Cancelled);}
        let owners=||owners_cancelled(left,right,&latch);
        verify(&owners)?;
        if owners(){return Err(ModelTileError::Cancelled);}
        Ok(ModelBankExposureRecoveryEvidence{primary,recovery})
    })();
    if result.is_err() && stopped(){return Err(ModelTileError::Cancelled);}
    if owners_cancelled(left,right,&latch){Err(ModelTileError::Cancelled)}else{result}
}

/// Explicit exhaustive collection of ordinary-bank then calibrated recovery.
/// Totals prepay `max_pairs` complete attempts, including skipped suffixes.
#[derive(Debug,Clone,Copy)]
pub struct ModelBankExposureCollectionPolicy {
    pub search:ModelBankExposureRecoveryPolicy,
    pub max_files:usize,
    pub max_pairs:usize,
    pub work:ModelTileWorkLimits,
    pub maximum_calibration_pixel_reads:u64,
}
impl ModelBankExposureCollectionPolicy {
    pub fn for_pairs(search:ModelBankExposureRecoveryPolicy,max_files:usize,max_pairs:usize)
        ->Result<Self,ModelTileError> {
        let count=u64::try_from(max_pairs).map_err(|_|ModelTileError::Work)?;
        let mul=|v:u64|v.checked_mul(count).ok_or(ModelTileError::Work);let p=search.work;
        Ok(Self{search,max_files,max_pairs,maximum_calibration_pixel_reads:mul(search.maximum_calibration_pixel_reads)?,
            work:ModelTileWorkLimits{
                model_input_reads:mul(p.model_input_reads)?,proposal_ranking_checks:mul(p.proposal_ranking_checks)?,
                proposal_separation_checks:mul(p.proposal_separation_checks)?,geometry_hypotheses:mul(p.geometry_hypotheses)?,
                point_checks:mul(p.point_checks)?,selection_checks:mul(p.selection_checks)?,
                rank_sites:mul(p.rank_sites)?,pixel_reads:mul(p.pixel_reads)?,
            }})
    }
}
pub type ModelBankExposureCollectionReport=crate::collection::CollectionReport<ModelBankExposureRecoveryEvidence>;
pub type ModelBankExposureCollectionError=crate::collection::CollectionError<ModelTileError>;

/// Examine every unordered pair, or every pair in a complete explicit schedule.
/// IDs determine source/target direction, including the asymmetric source-region
/// recovery recipe. No opposite-direction attempt or transitive grouping is implied.
/// All policies, stage and batch work, calibration and managed result slots are
/// admitted before consuming input. All files/dependencies, weights and request
/// owners remain guarded across the batch; any fatal error discards all results.
/// Inconclusive pairs and both-stage traces remain in the report. Provider/model
/// caches can span the batch. Paths, snapshot metadata and tensors remain outside
/// managed payload accounting; exhaustive enumeration is not a scale/RSS promise.
#[allow(clippy::too_many_arguments)]
pub fn scan_model_banks_exposure_recovery_collection_with(
    files:impl IntoIterator<Item=(u64,DecodeRequest)>,
    schedule:Option<&crate::pair_schedule::CompletePairSchedule>,weights:&Path,
    policy:ModelBankExposureCollectionPolicy,
    primary_recipes:&[ModelBankPixelRecipe],recovery_recipes:&[ModelBankPixelRecipe],
    budget:&MemoryBudget,provider:&mut Provider<'_>,cancel:impl Fn()->bool,
)->Result<ModelBankExposureCollectionReport,ModelBankExposureCollectionError> {
    use crate::collection::{CollectionError as E,CollectionSettings,scan_guarded_collection_ordered};
    scan_guarded_collection_ordered(files,weights,|_|{
        validate_exposure_recovery_policy(policy.search,primary_recipes,recovery_recipes,budget).map_err(E::Policy)?;
        let needed=ModelBankExposureCollectionPolicy::for_pairs(policy.search,policy.max_files,policy.max_pairs)
            .map_err(|_|E::Work)?;let a=needed.work;let b=policy.work;
        for (required,cap)in [(a.model_input_reads,b.model_input_reads),(a.proposal_ranking_checks,b.proposal_ranking_checks),
            (a.proposal_separation_checks,b.proposal_separation_checks),(a.geometry_hypotheses,b.geometry_hypotheses),
            (a.point_checks,b.point_checks),(a.selection_checks,b.selection_checks),(a.rank_sites,b.rank_sites),
            (a.pixel_reads,b.pixel_reads),(needed.maximum_calibration_pixel_reads,policy.maximum_calibration_pixel_reads)]{
            if required>cap{return Err(E::Work);}
        }
        Ok(CollectionSettings{max_files:policy.max_files,max_pairs:policy.max_pairs,
            max_model_bytes:policy.search.primary.comparison.max_model_bytes.max(policy.search.recovery.comparison.max_model_bytes),
            max_file_bytes:policy.search.primary.comparison.decode.max_file_bytes.max(policy.search.recovery.comparison.decode.max_file_bytes)})
    },budget,|left,right,cancel|search_model_banks_exposure_recovery_files_with(left,right,weights,
        policy.search,primary_recipes,recovery_recipes,budget,&mut *provider,cancel),schedule,cancel)
}

/// Two directed attempts for each unordered pair, with explicit transform direction.
#[derive(Debug, Clone, Copy)]
pub struct BidirectionalExposureEvidence {
    /// Canonical lower-ID source to higher-ID target.
    pub forward: ModelBankExposureRecoveryEvidence,
    /// Higher-ID source to lower-ID target; skipped after forward support.
    pub reverse: Option<ModelBankExposureRecoveryEvidence>,
}
impl BidirectionalExposureEvidence {
    pub fn supported(&self) -> bool {
        self.forward.supported() || self.reverse.is_some_and(|e| e.supported())
    }
}
pub type BidirectionalExposureCollectionReport =
    crate::collection::CollectionReport<BidirectionalExposureEvidence>;
impl ModelBankExposureCollectionPolicy {
    /// Reserve both complete directed searches for each unordered pair, even
    /// when forward support will allow the reverse search to be skipped.
    pub fn for_bidirectional_pairs(search:ModelBankExposureRecoveryPolicy,max_files:usize,max_pairs:usize)
        ->Result<Self,ModelTileError> {
        let attempts=max_pairs.checked_mul(2).ok_or(ModelTileError::Work)?;
        let mut policy=Self::for_pairs(search,max_files,attempts)?;
        policy.max_pairs=max_pairs;
        Ok(policy)
    }
}
/// Search both directions before retaining an inconclusive unordered pair.
/// Forward support skips reverse work, but both directions are admitted up front.
/// Evidence keeps each transform in its native direction; no matrix inversion or
/// transitive grouping is implied. Batch source/owner/weight guards remain atomic.
#[allow(clippy::too_many_arguments)]
pub fn scan_model_banks_bidirectional_exposure_recovery_collection_with(
    files:impl IntoIterator<Item=(u64,DecodeRequest)>,
    schedule:Option<&crate::pair_schedule::CompletePairSchedule>,weights:&Path,
    policy:ModelBankExposureCollectionPolicy,
    primary_recipes:&[ModelBankPixelRecipe],recovery_recipes:&[ModelBankPixelRecipe],
    budget:&MemoryBudget,provider:&mut Provider<'_>,cancel:impl Fn()->bool,
)->Result<BidirectionalExposureCollectionReport,ModelBankExposureCollectionError> {
    use crate::collection::{CollectionError as E,CollectionSettings,scan_guarded_collection_ordered};
    scan_guarded_collection_ordered(files,weights,|_|{
        validate_exposure_recovery_policy(policy.search,primary_recipes,recovery_recipes,budget).map_err(E::Policy)?;
        let needed=ModelBankExposureCollectionPolicy::for_bidirectional_pairs(policy.search,policy.max_files,policy.max_pairs)
            .map_err(|_|E::Work)?;let a=needed.work;let b=policy.work;
        for (required,cap)in [(a.model_input_reads,b.model_input_reads),(a.proposal_ranking_checks,b.proposal_ranking_checks),
            (a.proposal_separation_checks,b.proposal_separation_checks),(a.geometry_hypotheses,b.geometry_hypotheses),
            (a.point_checks,b.point_checks),(a.selection_checks,b.selection_checks),(a.rank_sites,b.rank_sites),
            (a.pixel_reads,b.pixel_reads),(needed.maximum_calibration_pixel_reads,policy.maximum_calibration_pixel_reads)]{
            if required>cap{return Err(E::Work);}
        }
        Ok(CollectionSettings{max_files:policy.max_files,max_pairs:policy.max_pairs,
            max_model_bytes:policy.search.primary.comparison.max_model_bytes.max(policy.search.recovery.comparison.max_model_bytes),
            max_file_bytes:policy.search.primary.comparison.decode.max_file_bytes.max(policy.search.recovery.comparison.decode.max_file_bytes)})
    },budget,|left,right,cancel|{
        let forward=search_model_banks_exposure_recovery_files_with(left,right,weights,
            policy.search,primary_recipes,recovery_recipes,budget,&mut *provider,cancel)?;
        let reverse=if forward.supported(){None}else{
            Some(search_model_banks_exposure_recovery_files_with(right,left,weights,
                policy.search,primary_recipes,recovery_recipes,budget,&mut *provider,cancel)?)
        };
        Ok(BidirectionalExposureEvidence{forward,reverse})
    },schedule,cancel)
}

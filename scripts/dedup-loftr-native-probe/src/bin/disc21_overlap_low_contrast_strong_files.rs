//! Real native LoFTR provider for the unified library lifecycle diagnostic.
//! Tensor allocations remain outside admission; cancellation is cooperative between kernels.
use rrrah_dedup::{
    anchor_rank::AnchorRankPolicy,
    animated::AnimationBudget,
    geometry::{GeometryPolicy, ProjectiveSamplingPolicy},
    local_rank::LocalRankPolicy,
    model_compare::ModelComparisonPolicy,
    model_input::ModelInputPolicy,

    proposals::ProposalPolicy,
    rank_region::RankRegionPolicy,
};
use std::error::Error;
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 5 {
        return Err("usage: disc21_overlap_low_contrast_strong_files source target weights max_side source_tolerance".into());
    }
    tch::set_num_threads(2);
    tch::manual_seed(173);
    let budget = rrrah_core::MemoryBudget::new(512 * 1024 * 1024);
    let policy = ModelComparisonPolicy {
        decode: AnimationBudget {
            max_frames: 1,
            max_pixels: 12800000,
            max_file_bytes: 5242880,
        },
        input: ModelInputPolicy {
            max_side: args[3].parse()?,
            max_source_pixels: 12800000,
            max_output_pixels: 409600,
            max_pixel_reads: 51200000,
        },
        proposals: ProposalPolicy {
            max_proposals: 5000,
            max_ranking_checks: 200000,
            max_separation_checks: 12497500,
            minimum_separation: 2.,
        },
        geometry: GeometryPolicy {
            tolerance: 2.,
            min_inliers: 20,
            max_points: 5000,
            max_hypotheses: 1000000,
        },
        sampling: ProjectiveSamplingPolicy {
            trials: 1000000,
            seed: 173,
        },
        anchors: AnchorRankPolicy {
            local: LocalRankPolicy {
                rank: RankRegionPolicy {
                    radius: 8,
                    minimum_contrast: 0.0005,
                    minimum_pairs: 1000,
                    maximum_sites: 80000000,
                    maximum_pixel_reads: 400000000,
                },
                filter_radius: 12,
                minimum_witnesses: 20,
                maximum_points: 28000,
                maximum_point_checks: 392014000,
                minimum_point_separation: 2.,
                target_tolerance: 2.,
                source_tolerance: args[4].parse()?,
                minimum_coverage: 0.3,
                minimum_agreement: 0.9,
            },
            window_radius: 2,
            maximum_selection_checks: 1176014000,
        },
        max_model_bytes: 128 * 1024 * 1024,
    };
    let mut left=rrrah_decode::DecodeRequest::new(&args[0]);left.assume_untagged_linear_srgb=true;
    let mut right=rrrah_decode::DecodeRequest::new(&args[1]);right.assume_untagged_linear_srgb=true;
    let n=25*policy.proposals.max_proposals as u64;
    let fit=2*policy.geometry.max_points as u64*(policy.sampling.trials+1);
    let sep=n*(n-1)/2;
    let mut target=policy.geometry;target.min_inliers=4;
    let banks=rrrah_dedup::joint_witnesses::JointBankPolicy {
        hypothesis:rrrah_dedup::geometry::JointProjectivePolicy{target,source_tolerance:policy.anchors.local.source_tolerance,maximum_projection_checks:fit},
        sampling:policy.sampling,
        witnesses:rrrah_dedup::joint_witnesses::JointWitnessPolicy{source_tolerance:policy.anchors.local.source_tolerance,target_tolerance:2.,minimum_separation:2.,maximum_points:n as usize,maximum_projection_checks:2*n,maximum_separation_checks:sep},
        minimum_witnesses:20,maximum_banks:25,maximum_total_projection_checks:25*(fit+2*n),maximum_total_separation_checks:25*sep,
    };
    let mut work=rrrah_dedup::model_tiles::ModelTileWorkLimits::for_model(policy)?;
    work.model_input_reads=52*policy.input.max_pixel_reads;
    work.point_checks*=2;work.selection_checks*=2;work.rank_sites*=2;work.pixel_reads*=2;

    let output={
      let l=rrrah_dedup::model_input_file::prepare_gray_model_file(&left,policy.decode,policy.input,&budget,||false)?;
      let r=rrrah_dedup::model_input_file::prepare_gray_model_file(&right,policy.decode,policy.input,&budget,||false)?;
      let model=rrrah_dedup::exact::ContentSnapshot::read(std::path::Path::new(&args[2]),policy.max_model_bytes,&||false)?;
      let guard=||->Result<(),Box<dyn Error>>{l.verify_source(||false)?;r.verify_source(||false)?;model.verify(&||false)?;Ok(())};
      let ld=l.original_dimensions();let rd=r.original_dimensions();let width=ld[0]/2;let height=ld[1]/2;
      if width==0 || height==0 {return Err("image too small".into());}
      assert!(work.model_input_reads>=20*policy.input.max_pixel_reads);
      assert!(work.geometry_hypotheses>=9*policy.sampling.trials);
      assert!(work.point_checks>=36*policy.anchors.local.maximum_point_checks);
      assert!(work.selection_checks>=36*policy.anchors.maximum_selection_checks);
      assert!(work.rank_sites>=36*policy.anchors.local.rank.maximum_sites);
      assert!(work.pixel_reads>=36*policy.anchors.local.rank.maximum_pixel_reads);
      let mut owned=Vec::new();let mut rectangles=Vec::new();let mut details=Vec::new();
      for y in [0,ld[1]/4,ld[1]-height] {for x in [0,ld[0]/4,ld[0]-width] {
        let rectangle=[x,y,width,height];guard()?;
        let bank=rrrah_dedup::model_loftr::extract_loftr_region_points_files(&left,&right,std::path::Path::new(&args[2]),rectangle,[0,0,rd[0],rd[1]],policy,&budget,||false)?;
        guard()?;assert_eq!(bank.original_dimensions,[ld,rd]);assert_eq!(bank.model_digest,model.digest());
        let points:Vec<_>=bank.points.iter().map(|p|serde_json::json!({"source":p.source,"target":p.target,"source_bits":p.source.map(f64::to_bits),"target_bits":p.target.map(f64::to_bits)})).collect();
        details.push(serde_json::json!({"source_rectangle":rectangle,"raw_proposals":bank.raw_proposals,"points":points}));rectangles.push(rectangle);owned.push(bank);
      }}
      let refs:Vec<_>=owned.iter().map(|b|&b.points[..]).collect();let mut attempts=Vec::new();
      let candidate=rrrah_dedup::joint_witnesses::search_joint_bank_candidate_with(&refs,(ld[0],ld[1]),(rd[0],rd[1]),banks,&budget,||false,|candidate|->Result<bool,rrrah_dedup::model_compare::ModelComparisonError>{
        let witnesses:Vec<_>=candidate.witnesses.iter().map(|p|serde_json::json!({"source":p.source,"target":p.target})).collect();
        for rank_radius in [8,2] { for radius in [2,6] {
          let mut anchors=policy.anchors;anchors.window_radius=radius;anchors.local.rank.radius=rank_radius;
          let result=rrrah_dedup::anchor_rank_file::compare_anchor_rank_files(&left,&right,candidate.transform,&candidate.witnesses,policy.decode,anchors,&budget,||false);
          let (supported,description)=match result {
            Ok(e)=>(e.supported,format!("{e:?}")),
            Err(rrrah_dedup::anchor_rank_file::AnchorRankFileError::Rank(rrrah_dedup::local_rank::LocalRankError::InsufficientGeometry))=>(false,"insufficient_geometry".to_string()),
            Err(e)=>return Err(rrrah_dedup::model_compare::ModelComparisonError::Pixels(e)),
          };
          attempts.push(serde_json::json!({"bank":candidate.hypothesis_bank,"window_radius":radius,"rank_radius":rank_radius,"transform":candidate.transform.matrix,"witnesses":witnesses,"pixels":description,"supported":supported}));
          if supported {return Ok(true);}
        }
        }
        Ok(false)
      })?;
      guard()?;
      serde_json::json!({"original_dimensions":[ld,rd],"source_rectangles":rectangles,"target_rectangle":[0,0,rd[0],rd[1]],"bank_count":owned.len(),"banks":details,"pixel_attempts":attempts,"supported":candidate.is_some(),"selected_bank":candidate.as_ref().map(|c|c.hypothesis_bank),"geometry_inliers":candidate.as_ref().map(|c|c.witnesses.len()),"scope":"Nine automatic50%-overlapping half-image source windows versus whole reference. Candidate1m/seed173/tolerances2/contrast0.0005/agreement0.9,min20 anchors with rank8/2. Selected after positive development observations; negative precision remains unqualified. Outer original-file/weights guards; JSON/Vec diagnostic scratch and model RSS external. Not full collection/cohort/precision qualification."})
    };
    assert_eq!(budget.used(),0);
    println!("{}",serde_json::json!({"evidence":output,"managed_used":budget.used(),"managed_peak":budget.peak()}));Ok(())
}

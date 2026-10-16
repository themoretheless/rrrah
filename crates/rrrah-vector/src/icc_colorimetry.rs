//! ICC matrix/TRC profile colorants using the conventional PCS D50 white.
//! <https://www.color.org/security/malformed/bad-illuminant/>
use moxcms::{ColorProfile, Matrix3d, XyY, Xyz, Xyzd};

/// Set validated RGB/white xy coordinates and adapt colorants to ICC PCS D50.
/// Returns None for invalid or singular chromaticities, leaving the profile alone.
pub fn set_rgb_colorimetry(profile: &mut ColorProfile, xy: [f64; 8]) -> Option<()> {
    rrrah_core::RgbChromaticities::new(xy)?;
    let white = XyY::new(xy[6], xy[7], 1.);
    let primaries = [
        XyY::new(xy[0], xy[1], 1.).to_xyzd(),
        XyY::new(xy[2], xy[3], 1.).to_xyzd(),
        XyY::new(xy[4], xy[5], 1.).to_xyzd(),
    ];
    let rgb = ColorProfile::rgb_to_xyz_d(
        Matrix3d {
            v: [
                [primaries[0].x, primaries[1].x, primaries[2].x],
                [primaries[0].y, primaries[1].y, primaries[2].y],
                [primaries[0].z, primaries[1].z, primaries[2].z],
            ],
        },
        white.to_xyzd(),
    );
    let pcs = Xyz {
        x: 0.9642,
        y: 1.,
        z: 0.8249,
    };
    let matrix = moxcms::adapt_to_illuminant_d(rgb, white, pcs);
    let adaptation = moxcms::adapt_to_illuminant_d(
        Matrix3d {
            v: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
        },
        white,
        pcs,
    );
    if matrix
        .v
        .iter()
        .flatten()
        .chain(adaptation.v.iter().flatten())
        .any(|v| !v.is_finite())
    {
        return None;
    }
    profile.red_colorant = Xyzd {
        x: matrix.v[0][0],
        y: matrix.v[1][0],
        z: matrix.v[2][0],
    };
    profile.green_colorant = Xyzd {
        x: matrix.v[0][1],
        y: matrix.v[1][1],
        z: matrix.v[2][1],
    };
    profile.blue_colorant = Xyzd {
        x: matrix.v[0][2],
        y: matrix.v[1][2],
        z: matrix.v[2][2],
    };
    profile.white_point = Xyzd {
        x: 0.9642,
        y: 1.,
        z: 0.8249,
    };
    profile.media_white_point = Some(profile.white_point);
    profile.chromatic_adaptation = Some(adaptation);
    profile.cicp = None;
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn synthesized_profiles_encode_icc_pcs_and_map_neutral_to_its_white() {
        for xy in [
            [0.64, 0.33, 0.30, 0.60, 0.15, 0.06, 0.3127, 0.3290],
            [0.68, 0.32, 0.265, 0.69, 0.15, 0.06, 0.3127, 0.3290],
            [0.67, 0.32, 0.21, 0.71, 0.14, 0.08, 0.34567, 0.35850],
        ] {
            let mut profile = ColorProfile::new_srgb();
            set_rgb_colorimetry(&mut profile, xy).unwrap();
            let matrix = profile.rgb_to_xyz_matrix();
            for (row, expected) in matrix.v.iter().zip([0.9642, 1., 0.8249]) {
                assert!((row.iter().sum::<f64>() - expected).abs() < 1e-6);
            }
            let encoded = profile.encode().unwrap();
            assert_eq!(
                &encoded[68..80],
                &[0, 0, 0xf6, 0xd6, 0, 1, 0, 0, 0, 0, 0xd3, 0x2d]
            );
        }
    }
    #[test]
    fn invalid_chromaticities_leave_existing_profile_intact() {
        let base = [0.64, 0.33, 0.30, 0.60, 0.15, 0.06, 0.3127, 0.3290];
        for index in 0..8 {
            for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                let mut xy = base;
                xy[index] = value;
                let mut profile = ColorProfile::new_srgb();
                let original = profile.rgb_to_xyz_matrix().v;
                assert!(set_rgb_colorimetry(&mut profile, xy).is_none());
                assert_eq!(profile.rgb_to_xyz_matrix().v, original);
            }
        }
        let mut duplicate = base;
        duplicate[2] = duplicate[0];
        duplicate[3] = duplicate[1];
        let mut profile = ColorProfile::new_srgb();
        let original = profile.rgb_to_xyz_matrix().v;
        assert!(set_rgb_colorimetry(&mut profile, duplicate).is_none());
        assert_eq!(profile.rgb_to_xyz_matrix().v, original);
    }
}

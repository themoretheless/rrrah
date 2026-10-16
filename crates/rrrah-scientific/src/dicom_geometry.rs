//! DICOM spacing values are row(vertical), column(horizontal).
//! Raster metadata instead stretches horizontal coordinates by width/height.
use super::{RasterDecodeError, bad, text};
#[derive(Default)]
pub(super) struct Geometry {
    spacing: [Option<[f64; 2]>; 3],
    ratio: Option<[u32; 2]>,
}
impl Geometry {
    pub(super) fn record(&mut self, tag: u32, vr: &[u8], value: &[u8]) -> Result<(), RasterDecodeError> {
        let mut fields = text(value)?.split('\\');
        let pair = [
            fields.next().ok_or_else(|| bad("missing spacing value"))?,
            fields.next().ok_or_else(|| bad("missing spacing value"))?,
        ];
        if fields.next().is_some() {
            return Err(bad("spacing multiplicity"));
        }
        if tag == 0x00280034 {
            if vr != b"IS" {
                return Err(bad("pixel aspect VR"));
            }
            let mut result = [0u32; 2];
            for (out, value) in result.iter_mut().zip(pair) {
                if value.len() > 12 {
                    return Err(bad("pixel aspect IS length"));
                }
                *out = value
                    .trim()
                    .parse()
                    .map_err(|_| bad("invalid pixel aspect integer"))?;
                if *out == 0 || *out > i32::MAX as u32 {
                    return Err(bad("pixel aspect integer range"));
                }
            }
            self.ratio = Some(result);
        } else {
            if vr != b"DS" {
                return Err(bad("pixel spacing VR"));
            }
            let mut result = [0f64; 2];
            for (out, value) in result.iter_mut().zip(pair) {
                if value.len() > 16 {
                    return Err(bad("spacing DS length"));
                }
                *out = value.trim().parse().map_err(|_| bad("invalid spacing decimal"))?;
                if !out.is_finite() || *out < 0. {
                    return Err(bad("invalid pixel spacing"));
                }
            }
            let index = match tag {
                0x00280030 => 0,
                0x00181164 => 1,
                0x00182010 => 2,
                _ => return Err(bad("unknown spacing tag")),
            };
            self.spacing[index] = Some(result);
        }
        Ok(())
    }
    pub(super) fn aspect(&self, width: u32, height: u32) -> Result<Option<f32>, RasterDecodeError> {
        let mut selected = None;
        for [row, column] in self.spacing.into_iter().flatten() {
            if (row == 0. && height != 1) || (column == 0. && width != 1) {
                return Err(bad("zero spacing requires a singleton dimension"));
            }
            // A legal singleton zero does not define the pixel's physical extent.
            // It cannot supply a finite display ratio; continue to another source.
            if selected.is_none() && row > 0. && column > 0. {
                selected = Some(column / row);
            }
        }
        if selected.is_none() {
            selected = self
                .ratio
                .map(|[vertical, horizontal]| f64::from(horizontal) / f64::from(vertical));
        }
        let Some(value) = selected else {
            return Ok(None);
        };
        let value = value as f32;
        if !value.is_finite()
            || value <= 0.
            || !(width as f32 * value).is_finite()
            || width as f32 * value == 0.
        {
            return Err(bad("unrepresentable pixel aspect geometry"));
        }
        Ok(Some(value))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn direction_precedence_singletons_and_strict_multiplicity() {
        let mut g = Geometry::default();
        g.record(0x00280034, b"IS", b"3\\2 ").unwrap();
        assert_eq!(g.aspect(3, 2).unwrap(), Some(2. / 3.));
        g.record(0x00182010, b"DS", b".75\\1.5 ").unwrap();
        assert_eq!(g.aspect(3, 2).unwrap(), Some(2.));
        g.record(0x00181164, b"DS", b"4\\3 ").unwrap();
        assert_eq!(g.aspect(3, 2).unwrap(), Some(0.75));
        g.record(0x00280030, b"DS", b"2\\3 ").unwrap();
        assert_eq!(g.aspect(3, 2).unwrap(), Some(1.5));
        let mut zero = Geometry::default();
        zero.record(0x00280030, b"DS", b"0\\3 ").unwrap();
        assert_eq!(zero.aspect(3, 1).unwrap(), None);
        assert!(zero.aspect(3, 2).is_err());
        zero.record(0x00280034, b"IS", b"2\\3 ").unwrap();
        assert_eq!(zero.aspect(3, 1).unwrap(), Some(1.5));
        for value in [
            b"1".as_slice(),
            b"1\\2\\3",
            b"0\\0",
            b"-1\\2",
            b"2147483648\\1",
            b"1.5\\2",
            b"1e1\\2",
            b"\\2",
        ] {
            assert!(Geometry::default().record(0x00280034, b"IS", value).is_err());
        }
        for value in [
            b"1".as_slice(),
            b"1\\2\\3",
            b"NaN\\2",
            b"inf\\2",
            b"-1\\2",
            b"\\2",
        ] {
            assert!(Geometry::default().record(0x00280030, b"DS", value).is_err());
        }
        let mut huge = Geometry::default();
        huge.record(0x00280030, b"DS", b"1e-300\\1e300").unwrap();
        assert!(huge.aspect(3, 2).is_err());
        assert!(Geometry::default().record(0x00280030, b"IS", b"1\\2 ").is_err());
        assert!(Geometry::default().record(0x00280034, b"DS", b"1\\2 ").is_err());
    }
}

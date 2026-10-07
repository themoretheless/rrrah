use crate::CudaError;

pub(crate) fn copy_chunks(
    output: &mut [[f32; 4]],
    address: u64,
    cancelled: &mut impl FnMut() -> bool,
    mut copy: impl FnMut(&mut [[f32; 4]], u64) -> Result<(), CudaError>,
) -> Result<(), CudaError> {
    let bytes = u64::try_from(std::mem::size_of_val(output))
        .map_err(|_| CudaError::Invalid("readback size overflow"))?;
    address
        .checked_add(bytes)
        .ok_or(CudaError::Invalid("readback address overflow"))?;
    let mut offset = 0;
    for chunk in output.chunks_mut(4096) {
        if cancelled() {
            return Err(CudaError::Cancelled);
        }
        copy(chunk, address + offset)?;
        offset += u64::try_from(std::mem::size_of_val(chunk))
            .map_err(|_| CudaError::Invalid("readback size overflow"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn readback_offsets_tail_cancellation_and_overflow() {
        let mut output = vec![[0.0; 4]; 8200];
        let mut offsets = Vec::new();
        copy_chunks(&mut output, 1024, &mut || false, |chunk, address| {
            offsets.push((address, chunk.len()));
            chunk.fill([1.0; 4]);
            Ok(())
        })
        .unwrap();
        assert_eq!(offsets, [(1024, 4096), (66560, 4096), (132_096, 8)]);
        assert!(
            output
                .iter()
                .all(|p| p.iter().all(|v| v.to_bits() == 1.0f32.to_bits()))
        );
        output.fill([0.0; 4]);
        let mut polls = 0;
        assert!(matches!(
            copy_chunks(
                &mut output,
                1024,
                &mut || {
                    polls += 1;
                    polls == 2
                },
                |chunk, _| {
                    chunk.fill([1.0; 4]);
                    Ok(())
                }
            ),
            Err(CudaError::Cancelled)
        ));
        assert!(
            output[..4096]
                .iter()
                .all(|p| p.iter().all(|v| v.to_bits() == 1.0f32.to_bits()))
        );
        assert!(
            output[4096..]
                .iter()
                .all(|p| p.iter().all(|v| v.to_bits() == 0.0f32.to_bits()))
        );
        assert!(matches!(
            copy_chunks(&mut output, u64::MAX, &mut || false, |_, _| panic!(
                "overflow must precede copying"
            )),
            Err(CudaError::Invalid(_))
        ));
    }
}

/// Borrow input in at most 4 MiB transfers; cancellation never copies a tail
/// or permits the caller to proceed to kernel launch after a cancelled chunk.
pub(crate) fn copy_input_chunks(
    input: &[[f32; 4]],
    address: u64,
    cancelled: &mut impl FnMut() -> bool,
    mut copy: impl FnMut(&[[f32; 4]], u64) -> Result<(), CudaError>,
) -> Result<(), CudaError> {
    let bytes = u64::try_from(std::mem::size_of_val(input))
        .map_err(|_| CudaError::Invalid("upload size overflow"))?;
    address
        .checked_add(bytes)
        .ok_or(CudaError::Invalid("upload address overflow"))?;
    let mut offset = 0;
    for chunk in input.chunks(262_144) {
        if cancelled() {
            return Err(CudaError::Cancelled);
        }
        copy(chunk, address + offset)?;
        offset += std::mem::size_of_val(chunk) as u64;
    }
    Ok(())
}

#[cfg(test)]
mod upload_tests {
    use super::*;
    #[test]
    fn bounded_upload_borrows_source_preserves_tail_and_stops_on_cancel_or_error() {
        let input = vec![[-0.0, 4.0, -2.0, 0.5]; 262_147];
        let mut calls = Vec::new();
        copy_input_chunks(&input, 128, &mut || false, |chunk, address| {
            calls.push((address, chunk.len(), chunk.as_ptr()));
            assert_eq!(chunk[0][0].to_bits(), (-0.0f32).to_bits());
            Ok(())
        })
        .unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!((calls[0].0, calls[0].1), (128, 262_144));
        assert_eq!(calls[0].2, input.as_ptr());
        assert_eq!((calls[1].0, calls[1].1), (4_194_432, 3));
        assert_eq!(calls[1].2, input[262_144..].as_ptr());
        let mut polls = 0;
        let mut copied = 0;
        assert!(matches!(
            copy_input_chunks(
                &input,
                128,
                &mut || {
                    polls += 1;
                    polls == 2
                },
                |chunk, _| {
                    copied += chunk.len();
                    Ok(())
                }
            ),
            Err(CudaError::Cancelled)
        ));
        assert_eq!(copied, 262_144);
        let mut calls = 0;
        assert!(matches!(
            copy_input_chunks(&input, 128, &mut || false, |_, _| {
                calls += 1;
                Err(CudaError::Driver("transfer failed".into()))
            }),
            Err(CudaError::Driver(_))
        ));
        assert_eq!(calls, 1);
        assert!(matches!(
            copy_input_chunks(&input, u64::MAX, &mut || false, |_, _| {
                panic!("overflow precedes copying")
            }),
            Err(CudaError::Invalid(_))
        ));
    }
}

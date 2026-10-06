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

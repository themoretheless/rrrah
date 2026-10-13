"""Apply strip-local padding to an isolated pinned rrrah LoFTR fork."""
from pathlib import Path
import sys

root = Path(sys.argv[1])
p = root / 'src/backbone.rs'
s = p.read_text()
old = '''        // Include the global zero border once. Adjacent output strips read
        // overlapping input halos, so strip boundaries introduce no padding.
        let padded = input.f_constant_pad_nd([1, 1, 1, 1])?;
'''
assert s.count(old) == 1
s = s.replace(old, '')
old = '            let strip = padded.f_narrow(2, first * self.stride, (rows - 1) * self.stride + 3)?;'
assert s.count(old) == 1
s = s.replace(old, '            let strip = padded_input_strip(input, first, rows, self.stride)?;')
helper = '''// Copy only the required rows and halo. Zero padding belongs exclusively
// to global image edges; an interior strip reads real neighboring rows.
fn padded_input_strip(input: &Tensor, first: i64, rows: i64, stride: i64) -> Result<Tensor, LoftrError> {
    let height = input.size()[2];
    let start = first * stride - 1;
    let end = (first + rows - 1) * stride + 2;
    let clipped_start = start.max(0);
    let clipped_end = end.min(height);
    Ok(input.f_narrow(2, clipped_start, clipped_end - clipped_start)?
        .f_constant_pad_nd([1, 1, (-start).max(0), (end - height).max(0)])?)
}

'''
assert s.count('impl TiledConv3x3 {') == 1
s = s.replace('impl TiledConv3x3 {', helper + 'impl TiledConv3x3 {')
p.write_text(s)
p = root / 'src/backbone/tests.rs'
with p.open('a') as f:
    f.write('''
#[test]
fn strip_padding_matches_global_border_reference_with_bounded_copy() -> Result<(), LoftrError> {
    let _inference = tch::no_grad_guard();
    for (height, width, stride) in [(137, 96, 2), (83, 97, 1), (1, 1, 1), (2, 3, 2)] {
        let input = Tensor::arange(2 * 3 * height * width, (Kind::Float, Device::Cpu))
            .reshape([2, 3, height, width]);
        let global = input.f_constant_pad_nd([1, 1, 1, 1])?;
        let output_height = (height - 1) / stride + 1;
        let mut first = 0;
        while first < output_height {
            let rows = (output_height - first).min(32);
            let expected = global.f_narrow(2, first * stride, (rows - 1) * stride + 3)?;
            let actual = padded_input_strip(&input, first, rows, stride)?;
            assert!(actual.equal(&expected));
            assert_eq!(actual.size(), [2, 3, (rows - 1) * stride + 3, width + 2]);
            assert!(actual.numel() <= (2 * 3 * (31 * stride + 3) * (width + 2)) as usize);
            if height > 80 { assert!(actual.numel() < global.numel()); }
            first += rows;
        }
    }
    Ok(())
}
''')

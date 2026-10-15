#[derive(Clone,Copy)]
struct LinearRgbaView<'a>{width:u32,height:u32,samples:&'a[f32]}
impl LinearRgbaView<'_>{
    pub(crate) fn pixel(&self, x: u32, y: u32) -> Option<[u32; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let index = usize::try_from(u64::from(y) * u64::from(self.width) + u64::from(x)).ok()?;
        self.samples.as_chunks::<4>().0.get(index).map(canonical)
    }

}
fn canonical(rgba: &[f32; 4]) -> [u32; 4] {
    if rgba[3] == 0.0 {
        [0; 4]
    } else {
        rgba.map(|v| if v == 0.0 { 0 } else { v.to_bits() })
    }
}
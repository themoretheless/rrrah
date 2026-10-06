use hayro::{hayro_syntax::Pdf, hayro_interpret::InterpreterSettings, RenderCache, RenderSettings};
fn main() {
    let bytes = std::fs::read(std::env::args().nth(1).unwrap()).unwrap();
    let pdf = Pdf::new(bytes).unwrap();
    assert_eq!(pdf.pages().len(), 2);
    let cache = RenderCache::new();
    for (page,color) in pdf.pages().iter().zip([[255,0,0,255],[0,255,0,255]]) {
        let pixmap = hayro::render(page, &cache, &InterpreterSettings::default(), &RenderSettings::default());
        assert_eq!((pixmap.width(),pixmap.height()),(10,10));
        assert!(pixmap.data_as_u8_slice().chunks_exact(4).all(|p|p==color));
    }
    println!("two pages: exact authored opaque RGBA samples");
}

use photo_dna::Hash;
fn main() {
    std::panic::set_hook(Box::new(|_| {}));
    for (w,h) in [(0,0),(0,2),(1,1),(1,2),(2,1),(2,2),(8,8)] {
        let r=std::panic::catch_unwind(||Hash::from_rgb_pixels(w,h,vec![[0,0,0];(w*h) as usize]));
        println!("{w}x{h}: {}", match r { Err(_)=>"panic",Ok(Err(_))=>"typed_error",Ok(Ok(_))=>"ok" });
    }
    let a=Hash::from_rgb_pixels(8,8,vec![[0,0,0];64]).unwrap();
    let b=Hash::from_rgb_pixels(8,8,vec![[255,255,255];64]).unwrap();
    println!("black_white_same_hash: {}",a==b);
}

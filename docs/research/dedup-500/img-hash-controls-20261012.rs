use img_hash::{HasherConfig,HashAlg,ImageHash};
use img_hash::image::{GrayImage,ImageBuffer,Rgba};
fn main(){
 let a=ImageHash::<Box<[u8]>>::from_bytes(&[0]).unwrap();
 let b=ImageHash::<Box<[u8]>>::from_bytes(&[0,255]).unwrap();
 let hash=HasherConfig::new().hash_alg(HashAlg::Blockhash).to_hasher();
 let mut img=GrayImage::new(9,9);img.put_pixel(1,1,img_hash::image::Luma([255]));
 let fractional=hash.hash_image(&img);
 let transparent_a:ImageBuffer<Rgba<u8>,Vec<u8>>=ImageBuffer::from_pixel(8,8,Rgba([0,0,0,0]));
 let transparent_b=ImageBuffer::from_fn(8,8,|x,y|{let v=((x+y*8)*4)as u8;Rgba([v,v,v,0])});
 let gray=HasherConfig::new().hash_alg(HashAlg::Mean).to_hasher();
 let black=GrayImage::from_pixel(8,8,img_hash::image::Luma([0]));let white=GrayImage::from_pixel(8,8,img_hash::image::Luma([255]));
 println!("{{\"unequal_length_distance\":{},\"fractional_hash\":{:?},\"transparent_mean_distance\":{},\"transparent_block_distance\":{},\"black_white_mean_distance\":{}}}",a.dist(&b),fractional.as_bytes(),gray.hash_image(&transparent_a).dist(&gray.hash_image(&transparent_b)),hash.hash_image(&transparent_a).dist(&hash.hash_image(&transparent_b)),gray.hash_image(&black).dist(&gray.hash_image(&white)));
}

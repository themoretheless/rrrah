#[cfg(test)]
mod tests {
    #[test]
    fn bc1_opaque_565_endpoints_and_all_four_indices() {
        let block=[0x00,0xf8,0xe0,0x07,0xe4,0xe4,0xe4,0xe4];
        let mut pixels=[0u32;16];
        texture2ddecoder::decode_bc1(&block,4,4,&mut pixels).unwrap();
        assert_eq!(pixels,[0xffff0000,0xff00ff00,0xffaa5500,0xff55aa00].repeat(4).as_slice());
    }
    #[test]
    fn bc1_transparent_mode_preserves_black_gray_white_and_alpha() {
        let block=[0x00,0x00,0xff,0xff,0xe4,0xe4,0xe4,0xe4];
        let mut pixels=[0u32;16];
        texture2ddecoder::decode_bc1a(&block,4,4,&mut pixels).unwrap();
        assert_eq!(pixels,[0xff000000,0xffffffff,0xff7f7f7f,0x00000000].repeat(4).as_slice());
    }
    #[test]
    fn bc1_partial_block_has_exact_authored_pixels() {
        let block=[0x00,0xf8,0xe0,0x07,0xe4,0xe4,0xe4,0xe4];
        let mut pixels=[0u32;9];
        texture2ddecoder::decode_bc1(&block,3,3,&mut pixels).unwrap();
        assert_eq!(pixels,[0xffff0000,0xff00ff00,0xffaa5500].repeat(3).as_slice());
    }
}

#[cfg(test)]
mod opaque_contract {
    #[test]
    fn rgb_bc1_index_three_is_opaque_black_in_three_color_mode() {
        let mut pixels=[0u32;16];
        texture2ddecoder::decode_bc1(&[0,0,255,255,255,255,255,255],4,4,&mut pixels).unwrap();
        assert_eq!(pixels,[0xff000000;16]);
    }
}

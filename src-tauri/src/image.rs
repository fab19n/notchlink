//! Turning Windows bitmaps into PNG data URLs the webview can show.

use base64::Engine;
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, GetDC, GetDIBits, HBITMAP, ReleaseDC,
};

/// Reads a bitmap (which must not be selected into a DC) as top-down RGBA pixels.
/// Pixels with no alpha information come back fully opaque.
pub unsafe fn bitmap_rgba(bitmap: HBITMAP, width: u32, height: u32) -> Result<Vec<u8>, String> {
    unsafe {
        let mut header = BITMAPINFO::default();
        header.bmiHeader = BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width as i32,
            biHeight: -(height as i32), // negative = top-down rows
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        };
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        let dc = GetDC(None);
        let rows = GetDIBits(dc, bitmap, 0, height, Some(pixels.as_mut_ptr() as *mut _), &mut header, DIB_RGB_COLORS);
        ReleaseDC(None, dc);
        if rows == 0 {
            return Err("GetDIBits failed".into());
        }
        // BGRA -> RGBA.
        let has_alpha = pixels.chunks_exact(4).any(|p| p[3] != 0);
        for p in pixels.chunks_exact_mut(4) {
            p.swap(0, 2);
            if !has_alpha {
                p[3] = 255;
            }
        }
        Ok(pixels)
    }
}

pub fn png_data_url(rgba: &[u8], width: u32, height: u32) -> String {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Fastest);
    let mut writer = encoder.write_header().expect("png header");
    writer.write_image_data(rgba).expect("png data");
    writer.finish().expect("png finish");
    format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(out))
}

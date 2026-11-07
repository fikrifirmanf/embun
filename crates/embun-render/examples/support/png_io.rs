//! Reading and writing 8-bit RGBA PNGs for the examples.
#![allow(dead_code)]

use std::path::Path;

pub fn write(path: impl AsRef<Path>, rgba: &[u8], width: u32, height: u32) {
    let path = path.as_ref();
    let file = std::fs::File::create(path).unwrap_or_else(|e| panic!("create {}: {e}", path.display()));
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .and_then(|mut w| w.write_image_data(rgba))
        .unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
}

/// Returns tightly packed RGBA8 pixels. Gray, RGB and 16-bit PNGs are converted.
pub fn read(path: impl AsRef<Path>) -> (Vec<u8>, u32, u32) {
    let path = path.as_ref();
    let file = std::fs::File::open(path).unwrap_or_else(|e| panic!("open {}: {e}", path.display()));
    let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let mut buf = vec![0; reader.output_buffer_size().expect("image too large")];
    let info = reader.next_frame(&mut buf).unwrap_or_else(|e| panic!("decode {}: {e}", path.display()));
    let src = &buf[..info.buffer_size()];
    let rgba = match info.color_type {
        png::ColorType::Rgba => src.to_vec(),
        png::ColorType::Rgb => src.as_chunks::<3>().0.iter().flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => src.as_chunks::<2>().0.iter().flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(),
        png::ColorType::Grayscale => src.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => unreachable!("EXPAND turns indexed PNGs into RGB"),
    };
    (rgba, info.width, info.height)
}

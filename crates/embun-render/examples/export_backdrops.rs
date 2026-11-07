//! Writes the test backdrops as PNGs, so the macOS capture app can show the
//! same pictures that Embun's examples use.
//!
//! cargo run -p embun-render --example export_backdrops -- <out dir>
//! Writes {photos,checker,solid}-{light,dark}.png.

#[path = "support/backdrops.rs"]
mod backdrops;
#[path = "support/png_io.rs"]
mod png_io;

fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "backdrops".into()));
    std::fs::create_dir_all(&dir).expect("create output dir");
    for dark in [false, true] {
        let appearance = if dark { "dark" } else { "light" };
        let flat = if dark { [28, 28, 30] } else { [242, 242, 247] };
        let images = [
            ("photos", backdrops::photos(dark)),
            ("checker", backdrops::checker(dark)),
            ("solid", backdrops::solid(backdrops::WIDTH, backdrops::HEIGHT, flat)),
        ];
        for (name, image) in images {
            let path = dir.join(format!("{name}-{appearance}.png"));
            png_io::write(&path, &image.rgba, image.width, image.height);
            println!("wrote {}", path.display());
        }
    }
}

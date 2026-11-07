//! Renders the sample photo library without a window and writes PNGs.
//!
//! cargo run -p embun --example gallery -- [out-dir]
//!
//! Writes gallery-light.png, gallery-dark.png and gallery-menu.png at 2x scale.

#[path = "support/mod.rs"]
mod support;

use embun::{Appearance, Renderer, SNAPSHOT_FORMAT, Theme, Ui, snapshot};

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let (device, queue) = pollster::block_on(async {
        let adapter = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env())
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .expect("no GPU adapter");
        adapter.request_device(&wgpu::DeviceDescriptor::default()).await.expect("request device")
    });
    let mut renderer = Renderer::new(&device, &queue, SNAPSHOT_FORMAT);
    for (name, appearance, menu_open) in [("light", Appearance::Light, false), ("dark", Appearance::Dark, false), ("menu", Appearance::Light, true)] {
        let theme = Theme { appearance, ..Theme::default() };
        let mut library = support::library::Library::new();
        library.menu_open = menu_open;
        let mut ui = Ui::new(library, theme, [1100.0, 700.0]);
        let (pixels, width, height) = snapshot(&mut ui, &mut renderer, &device, &queue, 2.0);
        let path = format!("{out}/gallery-{name}.png");
        let file = std::fs::File::create(&path).expect("create png");
        let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header().expect("png header").write_image_data(&pixels).expect("png data");
        println!("wrote {path} ({width}x{height})");
    }
}

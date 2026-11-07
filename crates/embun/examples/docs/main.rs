//! Builds the docs site in `site/`: renders every example to PNGs in light and
//! dark, then fills the page templates with those images and the example code.
//!
//! cargo run -p embun --example docs -- site [--html-only]
//!
//! `--html-only` skips the GPU, so CI can check that the pages are up to date.

#[path = "../support/mod.rs"]
mod support;

mod html;
mod snippets;

use embun::{Appearance, Element, Image, Renderer, SNAPSHOT_FORMAT, Theme, Ui, snapshot};
use embun::{App, div, image};
use snippets::Msg;
use std::path::Path;
use support::counter::Counter;
use support::photos::landscape;

pub enum Source {
    Snippet(fn(&Theme) -> Element<Msg>),
    Counter,
}

pub struct Example {
    pub name: &'static str,
    pub alt: &'static str,
    pub size: [f32; 2],
    /// Glass shows only over content, so glass examples sit on a photo and the
    /// rest sit on a plain surface.
    pub photo: bool,
    pub source: Source,
}

pub const SNIPPETS: &str = include_str!("snippets.rs");
pub const COUNTER: &str = include_str!("../support/counter.rs");

pub fn examples() -> Vec<Example> {
    use Source::Snippet;
    vec![
        Example { name: "counter", alt: "A large number 0 above two buttons, Decrease and Increase", size: [420.0, 300.0], photo: false, source: Source::Counter },
        Example { name: "button_basic", alt: "A Cancel button and a blue Save button over a landscape", size: [360.0, 140.0], photo: true, source: Snippet(snippets::button_basic) },
        Example { name: "button_sizes", alt: "Four buttons from Small to Extra large", size: [640.0, 140.0], photo: true, source: Snippet(snippets::button_sizes) },
        Example { name: "icon_buttons", alt: "Three round buttons: plus, minus and a blue close button", size: [360.0, 140.0], photo: true, source: Snippet(snippets::icon_buttons) },
        Example { name: "toolbar", alt: "A pair of back and forward buttons beside a blue Share button", size: [420.0, 140.0], photo: true, source: Snippet(snippets::toolbar) },
        Example { name: "sidebar_list", alt: "A glass sidebar with a Library heading and three albums, All Photos selected", size: [420.0, 260.0], photo: true, source: Snippet(snippets::sidebar_list) },
        Example { name: "switches", alt: "Two switches, the first on and the second off", size: [360.0, 120.0], photo: false, source: Snippet(snippets::switches) },
        Example { name: "checkboxes", alt: "Two checkboxes, the first checked", size: [360.0, 150.0], photo: false, source: Snippet(snippets::checkboxes) },
        Example { name: "radios", alt: "Three radio buttons with Medium selected", size: [360.0, 190.0], photo: false, source: Snippet(snippets::radios) },
        Example { name: "sliders", alt: "A slider at about one third", size: [360.0, 120.0], photo: false, source: Snippet(snippets::sliders) },
        Example { name: "segmented_control", alt: "A segmented control with Day, Week and Month, Week selected", size: [360.0, 120.0], photo: false, source: Snippet(snippets::segmented_control) },
        Example { name: "layout_stack", alt: "Three tiles in a row above two tiles in a column", size: [360.0, 200.0], photo: false, source: Snippet(snippets::layout_stack) },
        Example { name: "layout_grow", alt: "A wide gray tile between two small blue tiles", size: [400.0, 120.0], photo: false, source: Snippet(snippets::layout_grow) },
        Example { name: "layout_wrap", alt: "Seven rounded tags wrapping onto several lines", size: [380.0, 150.0], photo: false, source: Snippet(snippets::layout_wrap) },
        Example { name: "layout_absolute", alt: "A card with one tile in its top left corner and one in its bottom right", size: [380.0, 200.0], photo: false, source: Snippet(snippets::layout_absolute) },
        Example { name: "style_card", alt: "A white rounded card titled Storage with a line of gray text", size: [380.0, 160.0], photo: false, source: Snippet(snippets::style_card) },
        Example { name: "style_gradient", alt: "A rounded rectangle with a gradient from orange to blue", size: [380.0, 160.0], photo: false, source: Snippet(snippets::style_gradient) },
        Example { name: "text_styles", alt: "Four lines of text from a large bold title down to a small caption", size: [380.0, 180.0], photo: false, source: Snippet(snippets::text_styles) },
        Example { name: "text_wrapping", alt: "A centered paragraph wrapped to three lines", size: [380.0, 150.0], photo: false, source: Snippet(snippets::text_wrapping) },
        Example { name: "image_scaled", alt: "A rounded square image with a red, green and blue gradient", size: [380.0, 180.0], photo: false, source: Snippet(snippets::image_scaled) },
        Example { name: "field_single", alt: "A text field holding the word Embun above an empty field with the placeholder Search", size: [380.0, 150.0], photo: false, source: Snippet(snippets::field_single) },
        Example { name: "field_multi", alt: "A text area with two lines of text, the second wrapping", size: [380.0, 170.0], photo: false, source: Snippet(snippets::field_multi) },
        Example { name: "list_rows", alt: "A list showing items 1 to 6 of a million", size: [380.0, 240.0], photo: false, source: Snippet(snippets::list_rows) },
        Example { name: "menu_example", alt: "A glass menu with two sort options, one checked, a separator and Share", size: [380.0, 220.0], photo: true, source: Snippet(snippets::menu_example) },
        Example { name: "dialog_example", alt: "A dimmed window with a dialog asking Discard changes with two buttons", size: [420.0, 260.0], photo: true, source: Snippet(snippets::dialog_example) },
        Example { name: "theme_palette", alt: "Five color swatches: background, surface, label, separator and accent", size: [420.0, 140.0], photo: false, source: Snippet(snippets::theme_palette) },
    ]
}

struct Preview {
    build: fn(&Theme) -> Element<Msg>,
    backdrop: Image,
    photo: bool,
}

impl App for Preview {
    type Message = Msg;

    fn update(&mut self, _message: Msg) {}

    fn view(&self, theme: &Theme) -> Element<Msg> {
        let content = (self.build)(theme);
        if self.photo {
            div().items_center().justify_center().child(image(self.backdrop.clone()).absolute().inset(0.0)).child(content)
        } else {
            div().items_center().justify_center().bg(theme.background()).child(content)
        }
    }
}

fn write_png(path: &Path, pixels: &[u8], width: u32, height: u32) {
    let file = std::fs::File::create(path).expect("create png");
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().expect("png header").write_image_data(pixels).expect("png data");
}

fn render_images(out: &Path) {
    let (device, queue) = pollster::block_on(async {
        let adapter = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env())
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .expect("no GPU adapter");
        adapter.request_device(&wgpu::DeviceDescriptor::default()).await.expect("request device")
    });
    let mut renderer = Renderer::new(&device, &queue, SNAPSHOT_FORMAT);
    for (index, example) in examples().iter().enumerate() {
        for (suffix, appearance) in [("light", Appearance::Light), ("dark", Appearance::Dark)] {
            let theme = Theme { appearance, ..Theme::default() };
            let (width, height) = (example.size[0], example.size[1]);
            let (pixels, w, h) = match example.source {
                Source::Counter => {
                    let mut ui = Ui::new(Counter { count: 0 }, theme, example.size);
                    snapshot(&mut ui, &mut renderer, &device, &queue, 2.0)
                }
                Source::Snippet(build) => {
                    let preview = Preview { build, backdrop: landscape(index as u32 + 1, width as u32, height as u32), photo: example.photo };
                    let mut ui = Ui::new(preview, theme, example.size);
                    snapshot(&mut ui, &mut renderer, &device, &queue, 2.0)
                }
            };
            let path = out.join("img").join(format!("{}-{suffix}.png", example.name));
            write_png(&path, &pixels, w, h);
            println!("wrote {} ({w}x{h})", path.display());
        }
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let out = std::path::PathBuf::from(args.next().unwrap_or_else(|| "site".into()));
    let html_only = args.any(|a| a == "--html-only");
    std::fs::create_dir_all(out.join("img")).expect("create site/img");
    if !html_only {
        render_images(&out);
    }
    html::build(&out);
}

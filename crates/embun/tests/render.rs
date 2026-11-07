//! Renders whole apps on the GPU and checks pixels. Needs a GPU adapter or a
//! software one such as llvmpipe; set WGPU_BACKEND to pick a backend.

use embun::*;

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

/// One device for every test; see the note in embun-render's GPU tests.
fn gpu() -> &'static Gpu {
    static GPU: std::sync::OnceLock<Gpu> = std::sync::OnceLock::new();
    GPU.get_or_init(new_gpu)
}

fn new_gpu() -> Gpu {
    let (device, queue) = pollster::block_on(async {
        let adapter = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env())
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .expect("these tests need a GPU adapter");
        adapter.request_device(&wgpu::DeviceDescriptor::default()).await.expect("request device")
    });
    Gpu { device, queue }
}

/// A fixed view, rendered at scale 1.
struct Static(fn(&Theme) -> Element<()>);

impl App for Static {
    type Message = ();
    fn update(&mut self, _: ()) {}
    fn view(&self, theme: &Theme) -> Element<()> {
        (self.0)(theme)
    }
}

fn render(gpu: &Gpu, theme: Theme, view: fn(&Theme) -> Element<()>) -> impl Fn(u32, u32) -> [u8; 3] {
    let mut renderer = Renderer::new(&gpu.device, &gpu.queue, SNAPSHOT_FORMAT);
    let mut ui = Ui::new(Static(view), theme, [200.0, 100.0]);
    let (pixels, width, _) = snapshot(&mut ui, &mut renderer, &gpu.device, &gpu.queue, 1.0);
    move |x, y| {
        let i = ((y * width + x) * 4) as usize;
        [pixels[i], pixels[i + 1], pixels[i + 2]]
    }
}

#[test]
fn the_background_follows_the_appearance() {
    let gpu = gpu();
    let light = render(gpu, Theme::default(), |_| div());
    let dark = render(gpu, Theme { appearance: Appearance::Dark, ..Theme::default() }, |_| div());
    assert_eq!(light(10, 10), [242, 242, 247]);
    assert_eq!(dark(10, 10), [28, 28, 30]);
}

#[test]
fn content_rects_images_and_text_are_drawn() {
    let gpu = gpu();
    let px = render(gpu, Theme::default(), |_| {
        let red = Image::from_rgba(2, 2, [255, 0, 0, 255].repeat(4));
        row()
            .gap_2()
            .p_2()
            .child(div().size(40.0, 40.0).bg(Color::rgb(0, 128, 0)))
            .child(image(red).size(40.0, 40.0))
            .child(text("Hg").text_size(40.0).text_color(Color::BLACK))
    });
    assert_eq!(px(28, 28), [0, 128, 0]);
    assert_eq!(px(76, 28), [255, 0, 0]);
    // Somewhere in the text box, glyph pixels are much darker than the background.
    let darkest = (104..150).flat_map(|x| (8..58).map(move |y| (x, y))).map(|(x, y)| px(x, y)[1]).min().unwrap();
    assert!(darkest < 80, "no glyph pixels found; darkest green {darkest}");
}

#[test]
fn a_prominent_button_is_accent_blue_with_a_white_label() {
    let gpu = gpu();
    let px = render(gpu, Theme::default(), |_| {
        column().items_center().justify_center().child(button("Save").size(ControlSize::ExtraLarge).prominent())
    });
    // Away from the label, the button body is the tinted glass: close to the
    // macOS capture of a prominent button over this background, (0, 133, 251).
    let body = px(100 - 30, 50);
    assert!(body[0] < 20 && body[1].abs_diff(133) <= 8 && body[2] >= 235, "body {body:?}");
    // The label adds white pixels in the middle.
    let brightest = (85..115).map(|x| px(x, 50)[0]).max().unwrap();
    assert!(brightest > 200, "no white label pixels; brightest red {brightest}");
}

#[test]
fn glass_shows_the_content_behind_it_frosted() {
    let gpu = gpu();
    // Black and white stripes, with a glass panel over the middle.
    let view = |_: &Theme| {
        div()
            .child(row().h_full().children((0..20).map(|i| div().w(10.0).h_full().bg(if i % 2 == 0 { Color::BLACK } else { Color::WHITE }))))
            .child(div().absolute().top(20.0).left(40.0).size(120.0, 60.0).rounded(12.0).glass())
    };
    let px = render(gpu, Theme::default(), view);
    // Outside the glass the stripes are sharp.
    assert_eq!(px(5, 5), [0, 0, 0]);
    assert_eq!(px(15, 5), [255, 255, 255]);
    // Inside, the frost pulls neighboring stripes toward each other.
    let (dark, bright) = (px(105, 50)[1] as i32, px(115, 50)[1] as i32);
    assert!(bright - dark < 120, "stripes under glass should be softened: {dark} vs {bright}");
    assert!(bright - dark > 0, "the glass should still show the stripes faintly: {dark} vs {bright}");
}

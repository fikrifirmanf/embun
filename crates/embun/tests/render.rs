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
    render_sized(gpu, theme, [200.0, 100.0], view)
}

fn render_sized(gpu: &Gpu, theme: Theme, size: [f32; 2], view: fn(&Theme) -> Element<()>) -> impl Fn(u32, u32) -> [u8; 3] {
    let mut renderer = Renderer::new(&gpu.device, &gpu.queue, SNAPSHOT_FORMAT);
    let mut ui = Ui::new(Static(view), theme, size);
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

#[test]
fn menu_glass_frosts_the_glass_beneath_it() {
    let gpu = gpu();
    // A blue prominent button, and a menu whose left half covers it.
    let view = |_: &Theme| {
        div()
            .child(div().absolute().top(30.0).left(20.0).child(button("Save").size(ControlSize::ExtraLarge).prominent()))
            .child(popover((), menu().absolute().top(20.0).left(40.0).w(140.0).h(60.0)))
    };
    let with_menu = render(gpu, Theme::default(), view);
    let without = render(gpu, Theme::default(), |_| {
        div().child(div().absolute().top(30.0).left(20.0).child(button("Save").size(ControlSize::ExtraLarge).prominent()))
    });
    // Outside the menu, the button looks the same either way.
    assert_eq!(with_menu(25, 48), without(25, 48));
    // Inside the menu over the button, the blue shows through the frost.
    let [r, _, b] = with_menu(60, 48);
    assert!(b as i32 - r as i32 > 40, "menu over the button should be bluish, got {:?}", with_menu(60, 48));
    // Inside the menu away from the button, it is not.
    let [r, _, b] = with_menu(170, 70);
    assert!((b as i32 - r as i32).abs() < 20, "menu away from the button should be neutral, got {:?}", with_menu(170, 70));
}

#[test]
fn switches_checkboxes_and_sliders_draw_their_state() {
    let gpu = gpu();
    let px = render_sized(gpu, Theme::default(), [200.0, 160.0], |theme| {
        column()
            .p_4()
            .gap_2()
            .child(switch(theme, true, ()))
            .child(switch(theme, false, ()))
            .child(checkbox(theme, "", true, ()))
            .child(slider(theme, 0.5, |_| ()).w(120.0))
    });
    // Rows are 22, 22, 22 and 28 px tall with 8 px gaps, starting at y = 16.
    let on = px(16 + 4, 16 + 11);
    assert!(on[0] < 30 && on[1].abs_diff(133) <= 10 && on[2] > 230, "an on switch is accent blue: {on:?}");
    let off = px(16 + 4, 16 + 22 + 8 + 11);
    assert!(off.iter().all(|&c| c > 190) && off[2] - off[0] < 20, "an off switch is light gray: {off:?}");
    let knob = px(16 + 38 - 11, 16 + 11);
    assert!(knob.iter().all(|&c| c > 245), "the on switch's knob is white at its right end: {knob:?}");
    let checked = px(16 + 3, 16 + 60 + 11);
    assert!(checked[2] > 230 && checked[0] < 40, "a checked box is accent blue: {checked:?}");
    // The slider at one half: accent fill left of the thumb, gray track to its right.
    let y = 16 + 3 * 30 + 14;
    let fill = px(16 + 8, y);
    let rest = px(16 + 112, y);
    assert!(fill[2] > 230 && fill[0] < 40, "filled part is accent blue: {fill:?}");
    assert!(rest[2] - rest[0] < 20, "unfilled part is gray: {rest:?}");
}

//! Browser build of `embun-render`. JavaScript owns the page (controls,
//! dragging, the backdrop image); this crate owns the GPU and draws the glass.
#![cfg(target_arch = "wasm32")]

use embun_render::{Backdrop, Frame, GlassLayers, GlassParams, GlassRenderer, GlassShape, MAX_SHAPES};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct EmbunView {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: GlassRenderer,
    backdrop: Option<Backdrop>,
    shapes: Vec<GlassShape>,
    params: GlassParams,
    viewport: [f32; 2],
    scale_factor: f32,
    backdrop_scale: f32,
    scroll: f32,
    backend: String,
}

#[wasm_bindgen]
impl EmbunView {
    /// Sets up the GPU on `canvas`: WebGPU when the browser has it, WebGL2 otherwise.
    pub async fn create(canvas: web_sys::HtmlCanvasElement) -> Result<EmbunView, JsValue> {
        console_error_panic_hook::set_once();
        let (width, height) = (canvas.width().max(1), canvas.height().max(1));

        let instance = wgpu::util::new_instance_with_webgpu_detection(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        })
        .await;
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
            .map_err(|e| JsValue::from_str(&format!("Could not use the canvas for drawing: {e}")))?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .map_err(|e| JsValue::from_str(&format!("No WebGPU or WebGL2 adapter: {e}")))?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("embun"),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits()),
                ..Default::default()
            })
            .await
            .map_err(|e| JsValue::from_str(&format!("Could not open the GPU device: {e}")))?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats.first().copied().ok_or("The canvas reports no usable color format")?;
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&device, &config);
        let renderer = GlassRenderer::new(&device, format);
        let backend = format!("{:?}", adapter.get_info().backend);

        Ok(EmbunView {
            surface,
            device,
            queue,
            config,
            renderer,
            backdrop: None,
            shapes: Vec::new(),
            params: GlassParams::default(),
            viewport: [width as f32, height as f32],
            scale_factor: 1.0,
            backdrop_scale: 1.0,
            scroll: 0.0,
            backend,
        })
    }

    /// "BrowserWebGpu" or "Gl".
    pub fn backend(&self) -> String {
        self.backend.clone()
    }

    /// Call when the canvas changes size. Sizes are CSS pixels.
    pub fn resize(&mut self, css_width: f32, css_height: f32, device_pixel_ratio: f32) {
        let dpr = device_pixel_ratio.max(0.5);
        self.viewport = [css_width.max(1.0), css_height.max(1.0)];
        self.scale_factor = dpr;
        self.config.width = (css_width * dpr).round().max(1.0) as u32;
        self.config.height = (css_height * dpr).round().max(1.0) as u32;
        self.surface.configure(&self.device, &self.config);
    }

    /// Tightly packed sRGB RGBA8 pixels, for example from `getImageData`.
    pub fn set_backdrop(&mut self, rgba: &[u8], width: u32, height: u32) -> Result<(), JsValue> {
        if width == 0 || height == 0 || rgba.len() != (width * height * 4) as usize {
            return Err(JsValue::from_str("Backdrop pixels must be width * height * 4 bytes"));
        }
        self.backdrop = Some(self.renderer.create_backdrop(&self.device, &self.queue, rgba, width, height));
        Ok(())
    }

    /// Six numbers per shape: center x, center y, half width, half height, radius, appearance.
    pub fn set_shapes(&mut self, data: &[f32]) {
        self.shapes = data
            .as_chunks::<6>()
            .0
            .iter()
            .take(MAX_SHAPES)
            .map(|s| GlassShape { center: [s[0], s[1]], half_size: [s[2], s[3]], radius: s[4], appearance: s[5] })
            .collect();
    }

    #[allow(clippy::too_many_arguments)]
    pub fn set_glass(
        &mut self,
        level: f32,
        frost: bool,
        lensing: bool,
        rim: bool,
        fringe: bool,
        shadow: bool,
        merge: bool,
        reduce_transparency: bool,
        focused: bool,
    ) {
        let layers = GlassLayers { frost, lensing, rim, fringe, shadow, merge };
        self.params = if focused {
            GlassParams::from_level(level, layers, reduce_transparency)
        } else {
            GlassParams::unfocused(level, layers, reduce_transparency)
        };
    }

    /// `scroll` in backdrop texels; `backdrop_scale` in texels per CSS pixel.
    pub fn set_scroll(&mut self, scroll: f32, backdrop_scale: f32) {
        self.scroll = scroll;
        self.backdrop_scale = backdrop_scale;
    }

    pub fn render(&mut self) -> Result<(), JsValue> {
        let Some(backdrop) = &self.backdrop else { return Ok(()) };
        let frame_texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            other => return Err(JsValue::from_str(&format!("Could not get a frame to draw: {other:?}"))),
        };
        let view = frame_texture.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let frame = Frame {
            viewport: self.viewport,
            scale_factor: self.scale_factor,
            backdrop_scale: self.backdrop_scale,
            scroll: self.scroll,
            shapes: &self.shapes,
            params: self.params,
        };
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("embun frame") });
        self.renderer.render(&self.queue, &mut encoder, &view, backdrop, &frame);
        self.queue.submit([encoder.finish()]);
        self.queue.present(frame_texture);
        Ok(())
    }
}

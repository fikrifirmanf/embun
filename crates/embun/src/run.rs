//! Runs an [`App`] in a native window with winit.

use std::sync::Arc;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::keyboard::{Key as WinitKey, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

use crate::{App, Appearance, Key, Renderer, TextSystem, Theme, Ui};

/// Window and appearance options for [`run`].
#[derive(Clone, Debug)]
pub struct Settings {
    pub title: String,
    /// Initial window size in logical pixels.
    pub size: [f32; 2],
    /// Light or dark; `None` follows the system and its changes.
    pub appearance: Option<Appearance>,
    /// The glass slider: 0 is clear, 0.5 the macOS default, 1 tinted.
    pub glass_level: f32,
    pub reduce_transparency: bool,
    /// Also load installed fonts, for emoji and scripts Inter lacks.
    pub system_fonts: bool,
}

impl Settings {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            size: [960.0, 640.0],
            appearance: None,
            glass_level: 0.5,
            reduce_transparency: false,
            system_fonts: false,
        }
    }

    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.size = [width, height];
        self
    }

    pub fn appearance(mut self, appearance: Appearance) -> Self {
        self.appearance = Some(appearance);
        self
    }

    pub fn glass_level(mut self, level: f32) -> Self {
        self.glass_level = level;
        self
    }

    pub fn reduce_transparency(mut self, on: bool) -> Self {
        self.reduce_transparency = on;
        self
    }

    pub fn system_fonts(mut self, on: bool) -> Self {
        self.system_fonts = on;
        self
    }
}

/// Why [`run`] stopped early.
#[derive(Debug)]
pub enum RunError {
    EventLoop(winit::error::EventLoopError),
    /// No GPU adapter could draw to the window.
    NoAdapter(String),
    Device(wgpu::RequestDeviceError),
    Window(String),
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunError::EventLoop(e) => write!(f, "event loop: {e}"),
            RunError::NoAdapter(e) => write!(f, "no GPU adapter can draw to this window: {e}"),
            RunError::Device(e) => write!(f, "could not open the GPU: {e}"),
            RunError::Window(e) => write!(f, "could not create the window: {e}"),
        }
    }
}

impl std::error::Error for RunError {}

/// Opens a window and runs `app` until the window closes.
pub fn run<A: App + 'static>(app: A, settings: Settings) -> Result<(), RunError> {
    let event_loop = EventLoop::<accesskit_winit::Event>::with_user_event().build().map_err(RunError::EventLoop)?;
    let proxy = event_loop.create_proxy();
    let mut shell = Shell { pending: Some((app, settings.clone())), settings, proxy, state: None, error: None };
    event_loop.run_app(&mut shell).map_err(RunError::EventLoop)?;
    match shell.error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

struct Shell<A: App> {
    pending: Option<(A, Settings)>,
    settings: Settings,
    /// Carries screen reader requests from AccessKit into the event loop.
    proxy: EventLoopProxy<accesskit_winit::Event>,
    state: Option<State<A>>,
    error: Option<RunError>,
}

struct State<A: App> {
    ui: Ui<A>,
    renderer: Renderer,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    instance: wgpu::Instance,
    modifiers: ModifiersState,
    /// Opened on first use; `None` if the system clipboard is unavailable.
    clipboard: Option<arboard::Clipboard>,
    accessibility: accesskit_winit::Adapter,
    last_frame: Instant,
    // Dropped last: the surface must go before its window.
    window: Arc<Window>,
}

impl<A: App + 'static> Shell<A> {
    fn start(&mut self, event_loop: &ActiveEventLoop) -> Result<(), RunError> {
        let Some((app, settings)) = self.pending.take() else { return Ok(()) };
        let attributes = Window::default_attributes()
            .with_title(settings.title.clone())
            .with_inner_size(LogicalSize::new(settings.size[0] as f64, settings.size[1] as f64))
            // Hidden until the accessibility adapter is attached, as AccessKit requires.
            .with_visible(false);
        let window = Arc::new(event_loop.create_window(attributes).map_err(|e| RunError::Window(e.to_string()))?);
        let accessibility = accesskit_winit::Adapter::with_event_loop_proxy(event_loop, &window, self.proxy.clone());
        window.set_visible(true);
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle_from_env(Box::new(
            event_loop.owned_display_handle(),
        )));
        let surface = instance.create_surface(window.clone()).map_err(|e| RunError::Window(e.to_string()))?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .map_err(|e| RunError::NoAdapter(e.to_string()))?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).map_err(RunError::Device)?;
        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats.iter().copied().find(|f| f.is_srgb()).unwrap_or(caps.formats[0]);
        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&device, &config);

        let scale = window.scale_factor() as f32;
        let appearance = settings.appearance.unwrap_or(match window.theme() {
            Some(winit::window::Theme::Dark) => Appearance::Dark,
            _ => Appearance::Light,
        });
        let theme = Theme {
            appearance,
            glass_level: settings.glass_level,
            focused: true,
            reduce_transparency: settings.reduce_transparency,
        };
        let text = if settings.system_fonts { TextSystem::with_system_fonts() } else { TextSystem::new() };
        let ui = Ui::with_text(app, theme, [size.width as f32 / scale, size.height as f32 / scale], text);
        let renderer = Renderer::new(&device, &queue, format);
        window.request_redraw();
        self.state = Some(State {
            ui,
            renderer,
            device,
            queue,
            surface,
            config,
            instance,
            modifiers: ModifiersState::empty(),
            clipboard: None,
            accessibility,
            last_frame: Instant::now(),
            window,
        });
        Ok(())
    }
}

impl<A: App + 'static> ApplicationHandler<accesskit_winit::Event> for Shell<A> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if let Err(e) = self.start(event_loop) {
            self.error = Some(e);
            event_loop.exit();
        }
    }

    fn user_event(&mut self, _: &ActiveEventLoop, event: accesskit_winit::Event) {
        let Some(s) = self.state.as_mut() else { return };
        let scale = s.window.scale_factor() as f32;
        match event.window_event {
            accesskit_winit::WindowEvent::InitialTreeRequested => {
                let ui = &mut s.ui;
                s.accessibility.update_if_active(|| ui.accessibility(scale));
            }
            accesskit_winit::WindowEvent::ActionRequested(request) => {
                s.ui.accessibility_action(&request);
                s.window.request_redraw();
            }
            accesskit_winit::WindowEvent::AccessibilityDeactivated => {}
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let follow_system = self.settings.appearance.is_none();
        let Some(s) = self.state.as_mut() else { return };
        s.accessibility.process_event(&s.window, &event);
        let scale = s.window.scale_factor() as f32;
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                s.config.width = size.width.max(1);
                s.config.height = size.height.max(1);
                s.surface.configure(&s.device, &s.config);
                s.ui.resize([size.width as f32 / scale, size.height as f32 / scale]);
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                let size = s.window.inner_size();
                s.ui.resize([size.width as f32 / scale, size.height as f32 / scale]);
            }
            WindowEvent::Focused(focused) => s.ui.set_theme(Theme { focused, ..*s.ui.theme() }),
            WindowEvent::ThemeChanged(theme) if follow_system => {
                let appearance = if theme == winit::window::Theme::Dark { Appearance::Dark } else { Appearance::Light };
                s.ui.set_theme(Theme { appearance, ..*s.ui.theme() });
            }
            WindowEvent::CursorMoved { position, .. } => s.ui.pointer_moved([position.x as f32 / scale, position.y as f32 / scale]),
            WindowEvent::CursorLeft { .. } => s.ui.pointer_left(),
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => match state {
                ElementState::Pressed => s.ui.pointer_pressed(),
                ElementState::Released => s.ui.pointer_released(),
            },
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y * 40.0,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / scale,
                };
                s.ui.scrolled(-dy);
            }
            WindowEvent::ModifiersChanged(m) => s.modifiers = m.state(),
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                let shift = s.modifiers.shift_key();
                // Command on macOS, Control elsewhere.
                let command = s.modifiers.super_key() || s.modifiers.control_key();
                let editing = s.ui.is_editing();
                let pick = |plain: Key, selecting: Key| Some(if shift { selecting } else { plain });
                let key = match &event.logical_key {
                    WinitKey::Named(NamedKey::Tab) => pick(Key::Tab, Key::ShiftTab),
                    WinitKey::Named(NamedKey::Escape) => Some(Key::Escape),
                    WinitKey::Named(NamedKey::Enter | NamedKey::Space) if !editing => Some(Key::Activate),
                    WinitKey::Named(NamedKey::Backspace) if editing => Some(Key::Backspace),
                    WinitKey::Named(NamedKey::Delete) if editing => Some(Key::Delete),
                    WinitKey::Named(NamedKey::ArrowLeft) if editing && command => pick(Key::Home, Key::SelectHome),
                    WinitKey::Named(NamedKey::ArrowRight) if editing && command => pick(Key::End, Key::SelectEnd),
                    WinitKey::Named(NamedKey::ArrowLeft) if editing => pick(Key::Left, Key::SelectLeft),
                    WinitKey::Named(NamedKey::ArrowRight) if editing => pick(Key::Right, Key::SelectRight),
                    WinitKey::Named(NamedKey::Home | NamedKey::ArrowUp) if editing => pick(Key::Home, Key::SelectHome),
                    WinitKey::Named(NamedKey::End | NamedKey::ArrowDown) if editing => pick(Key::End, Key::SelectEnd),
                    WinitKey::Character(c) if editing && command && c.eq_ignore_ascii_case("a") => Some(Key::SelectAll),
                    WinitKey::Character(c) if editing && command => {
                        clipboard(&mut s.clipboard, c, &mut s.ui);
                        None
                    }
                    _ => None,
                };
                if let Some(key) = key {
                    s.ui.key(key);
                } else if editing
                    && !command
                    && let Some(text) = &event.text
                {
                    s.ui.insert_text(text);
                }
            }
            // Text composed with an input method, such as Japanese or Chinese.
            WindowEvent::Ime(Ime::Commit(text)) => s.ui.insert_text(&text),
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = (now - s.last_frame).as_secs_f32().min(0.05);
                s.last_frame = now;
                let animating = s.ui.tick(dt);
                s.draw();
                if animating {
                    s.window.request_redraw();
                }
                return;
            }
            _ => {}
        }
        s.window.set_ime_allowed(s.ui.is_editing());
        if s.ui.needs_redraw() {
            s.window.request_redraw();
        }
    }
}

/// Command-C, -X and -V in a text field.
fn clipboard<A: App>(clipboard: &mut Option<arboard::Clipboard>, key: &str, ui: &mut Ui<A>) {
    if clipboard.is_none() {
        *clipboard = arboard::Clipboard::new().ok();
    }
    let Some(board) = clipboard.as_mut() else { return };
    let copied = match key.to_ascii_lowercase().as_str() {
        "c" => ui.selected_text(),
        "x" => ui.cut(),
        "v" => {
            if let Ok(text) = board.get_text() {
                ui.insert_text(&text);
            }
            None
        }
        _ => None,
    };
    if let Some(text) = copied
        && let Err(e) = board.set_text(text)
    {
        eprintln!("embun: could not copy to the clipboard: {e}");
    }
}

impl<A: App> State<A> {
    fn draw(&mut self) {
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return,
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                self.window.request_redraw();
                return;
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                match self.instance.create_surface(self.window.clone()) {
                    Ok(surface) => {
                        self.surface = surface;
                        self.surface.configure(&self.device, &self.config);
                        self.window.request_redraw();
                    }
                    Err(e) => eprintln!("embun: could not recreate the window surface: {e}"),
                }
                return;
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                eprintln!("embun: the window surface failed validation; skipping the frame");
                return;
            }
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("embun frame") });
        let scale = self.window.scale_factor() as f32;
        let changed = self.ui.update();
        self.ui.render(&mut self.renderer, &self.device, &self.queue, &mut encoder, &view, [self.config.width, self.config.height], scale);
        if changed {
            let ui = &mut self.ui;
            self.accessibility.update_if_active(|| ui.accessibility(scale));
        }
        self.queue.submit([encoder.finish()]);
        self.window.pre_present_notify();
        self.queue.present(frame);
    }
}

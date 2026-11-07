use std::collections::HashMap;
use std::sync::Arc;

use glyphon::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, Weight, fontdb};

use crate::element::TextAlign;

/// The font every label uses: Inter, embedded so text measures the same on
/// every machine. Licensed under the SIL Open Font License (fonts/OFL.txt).
pub const FONT_FAMILY: &str = "Inter";

const FONTS: [&[u8]; 4] = [
    include_bytes!("../fonts/Inter-Regular.ttf"),
    include_bytes!("../fonts/Inter-Medium.ttf"),
    include_bytes!("../fonts/Inter-SemiBold.ttf"),
    include_bytes!("../fonts/Inter-Bold.ttf"),
];

/// How a run of text is set.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TextStyle {
    pub size: f32,
    pub weight: u16,
    /// Line height as a multiple of the size.
    pub line_height: f32,
    pub align: TextAlign,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Key {
    text: String,
    size: u32,
    weight: u16,
    line_height: u32,
    align: u8,
    /// Wrap width in hundredths of a pixel, or `u32::MAX` for no wrapping.
    width: u32,
}

impl Key {
    fn new(text: &str, style: &TextStyle, width: Option<f32>) -> Self {
        Key {
            text: text.to_owned(),
            size: style.size.to_bits(),
            weight: style.weight,
            line_height: style.line_height.to_bits(),
            align: style.align as u8,
            width: width.map_or(u32::MAX, |w| (w.max(0.0) * 100.0).round() as u32),
        }
    }
}

/// Fonts and shaped text, shared by layout (to measure) and the renderer (to
/// draw). Shaped buffers are cached between frames.
pub struct TextSystem {
    pub(crate) fonts: FontSystem,
    cache: HashMap<Key, (Buffer, u64)>,
    frame: u64,
}

impl Default for TextSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl TextSystem {
    /// Loads only the embedded Inter fonts. Characters Inter lacks, such as
    /// emoji and CJK, are not drawn; see [`TextSystem::with_system_fonts`].
    pub fn new() -> Self {
        let mut db = fontdb::Database::new();
        for font in FONTS {
            db.load_font_source(fontdb::Source::Binary(Arc::new(font)));
        }
        db.set_sans_serif_family(FONT_FAMILY);
        Self { fonts: FontSystem::new_with_locale_and_db("en-US".into(), db), cache: HashMap::new(), frame: 0 }
    }

    /// Also loads the fonts installed on this machine, as fallbacks for
    /// characters Inter lacks. Slower to start, and text may measure
    /// differently from machine to machine.
    pub fn with_system_fonts() -> Self {
        let mut system = Self::new();
        system.fonts.db_mut().load_system_fonts();
        system
    }

    /// Size of `text` laid out within `max_width` (unbounded when `None`).
    pub(crate) fn measure(&mut self, text: &str, style: &TextStyle, max_width: Option<f32>) -> [f32; 2] {
        let buffer = self.buffer(text, style, max_width);
        let (mut width, mut height) = (0.0f32, 0.0f32);
        for run in buffer.layout_runs() {
            width = width.max(run.line_w);
            height = height.max(run.line_top + run.line_height);
        }
        if height == 0.0 {
            height = style.size * style.line_height;
        }
        [width.ceil(), height.ceil()]
    }

    /// The shaped buffer for `text` at `width`, from the cache when possible.
    pub(crate) fn buffer(&mut self, text: &str, style: &TextStyle, width: Option<f32>) -> &Buffer {
        let key = Key::new(text, style, width);
        let frame = self.frame;
        let fonts = &mut self.fonts;
        let entry = self.cache.entry(key).or_insert_with(|| {
            let mut buffer = Buffer::new(fonts, Metrics::new(style.size, style.size * style.line_height));
            buffer.set_size(width, None);
            let attrs = Attrs::new().family(Family::Name(FONT_FAMILY)).weight(Weight(style.weight));
            let align = match style.align {
                TextAlign::Left => None,
                TextAlign::Center => Some(glyphon::cosmic_text::Align::Center),
                TextAlign::Right => Some(glyphon::cosmic_text::Align::Right),
            };
            buffer.set_text(text, &attrs, Shaping::Advanced, align);
            buffer.shape_until_scroll(fonts, false);
            (buffer, frame)
        });
        entry.1 = frame;
        &entry.0
    }

    /// The font system and read access to the shaped buffers, borrowed
    /// together for drawing.
    pub(crate) fn split(&mut self) -> (&mut FontSystem, Buffers<'_>) {
        (&mut self.fonts, Buffers(&self.cache))
    }

    /// Drops buffers not used in the last few frames.
    pub(crate) fn end_frame(&mut self) {
        let frame = self.frame;
        self.cache.retain(|_, (_, used)| frame.saturating_sub(*used) < 3);
        self.frame += 1;
    }
}

/// Shaped buffers made earlier this frame by [`TextSystem::buffer`].
pub(crate) struct Buffers<'a>(&'a HashMap<Key, (Buffer, u64)>);

impl<'a> Buffers<'a> {
    pub(crate) fn get(&self, text: &str, style: &TextStyle, width: Option<f32>) -> Option<&'a Buffer> {
        self.0.get(&Key::new(text, style, width)).map(|(buffer, _)| buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn style(size: f32) -> TextStyle {
        TextStyle { size, weight: 400, line_height: 1.25, align: TextAlign::Left }
    }

    #[test]
    fn longer_text_measures_wider_and_bigger_text_taller() {
        let mut text = TextSystem::new();
        let [short, h13] = text.measure("Save", &style(13.0), None);
        let [long, _] = text.measure("Save as a copy", &style(13.0), None);
        let [_, h26] = text.measure("Save", &style(26.0), None);
        assert!(short > 10.0 && long > short * 2.0, "widths {short} and {long}");
        assert!(h26 > h13 * 1.8, "heights {h13} and {h26}");
    }

    #[test]
    fn text_wraps_to_the_given_width() {
        let mut text = TextSystem::new();
        let words = "one two three four five six seven eight nine ten";
        let [one_line_w, _] = text.measure(words, &style(13.0), None);
        let [w, h] = text.measure(words, &style(13.0), Some(one_line_w / 2.5));
        assert!(w <= one_line_w / 2.5 + 1.0, "wrapped width {w}");
        // At least three lines of 13 * 1.25 px.
        assert!(h >= 3.0 * 13.0 * 1.25, "wrapped height {h}");
    }
}

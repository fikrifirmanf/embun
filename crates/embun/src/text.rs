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

    /// Where the caret for byte `offset` of `text` sits when wrapped to
    /// `width`: x, and the top of its line, from the text's top-left.
    pub(crate) fn caret(&mut self, text: &str, style: &TextStyle, width: Option<f32>, offset: usize) -> [f32; 2] {
        let buffer = self.buffer(text, style, width);
        let starts = line_starts(text);
        let runs: Vec<_> = buffer.layout_runs().collect();
        let mut last = [0.0, 0.0];
        for (i, run) in runs.iter().enumerate() {
            let base = starts.get(run.line_i).copied().unwrap_or(0);
            let (start, end) = run_bounds(run, base);
            let last_of_line = runs.get(i + 1).is_none_or(|next| next.line_i != run.line_i);
            last = [x_in_run(run, base, end.min(offset.max(start))), run.line_top];
            if offset >= start && (offset < end || (offset == end && last_of_line)) {
                return [x_in_run(run, base, offset), run.line_top];
            }
        }
        last
    }

    /// The byte offset nearest to `point`, from the text's top-left.
    pub(crate) fn hit(&mut self, text: &str, style: &TextStyle, width: Option<f32>, point: [f32; 2]) -> usize {
        let buffer = self.buffer(text, style, width);
        let starts = line_starts(text);
        match buffer.hit(point[0], point[1]) {
            Some(cursor) => (starts.get(cursor.line).copied().unwrap_or(0) + cursor.index).min(text.len()),
            None if point[1] < 0.0 => 0,
            None => text.len(),
        }
    }

    /// Rectangles (x, y, width, height from the text's top-left) covering
    /// bytes `start..end`, one per line the selection touches. A selected
    /// line break is drawn as a short extension of its line.
    pub(crate) fn selection_rects(&mut self, text: &str, style: &TextStyle, width: Option<f32>, start: usize, end: usize) -> Vec<[f32; 4]> {
        let buffer = self.buffer(text, style, width);
        let starts = line_starts(text);
        let runs: Vec<_> = buffer.layout_runs().collect();
        let mut rects = Vec::new();
        for (i, run) in runs.iter().enumerate() {
            let base = starts.get(run.line_i).copied().unwrap_or(0);
            let (rs, re) = run_bounds(run, base);
            let last_of_line = runs.get(i + 1).is_none_or(|next| next.line_i != run.line_i);
            let (lo, hi) = (start.max(rs), end.min(re));
            let breaks = last_of_line && start <= re && end > re && re < text.len();
            if lo >= hi && !breaks {
                continue;
            }
            let x0 = x_in_run(run, base, lo.min(re));
            let x1 = x_in_run(run, base, hi.max(lo).min(re)) + if breaks { 4.0 } else { 0.0 };
            if x1 > x0 {
                rects.push([x0, run.line_top, x1 - x0, run.line_height]);
            }
        }
        rects
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

/// Byte offset where each `\n`-separated line of `text` starts.
fn line_starts(text: &str) -> Vec<usize> {
    let mut starts = vec![0];
    starts.extend(text.match_indices('\n').map(|(i, _)| i + 1));
    starts
}

/// The first and last byte (in the whole text) of what a layout run shows.
fn run_bounds(run: &glyphon::cosmic_text::LayoutRun<'_>, base: usize) -> (usize, usize) {
    match (run.glyphs.first(), run.glyphs.last()) {
        (Some(first), Some(last)) => (base + first.start.min(last.start), base + first.end.max(last.end)),
        _ => (base, base),
    }
}

/// Horizontal position of byte `at` inside a layout run.
fn x_in_run(run: &glyphon::cosmic_text::LayoutRun<'_>, base: usize, at: usize) -> f32 {
    for glyph in run.glyphs {
        if at < base + glyph.end {
            return glyph.x;
        }
    }
    run.glyphs.last().map_or(0.0, |g| g.x + g.w)
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
    fn the_caret_follows_lines_and_wrapping() {
        let mut text = TextSystem::new();
        let st = style(13.0);
        let line = 13.0 * 1.25;
        // Start of an empty text, and after a typed character.
        assert_eq!(text.caret("", &st, None, 0), [0.0, 0.0]);
        let [x_ab, y_ab] = text.caret("ab", &st, None, 2);
        assert!(x_ab > 5.0 && y_ab == 0.0, "{x_ab} {y_ab}");
        // After a newline the caret is at the left of the next line.
        let [x, y] = text.caret("ab\ncd", &st, None, 3);
        assert_eq!((x, y), (0.0, line));
        // At the end of the first line it is still on the first line.
        assert_eq!(text.caret("ab\ncd", &st, None, 2)[1], 0.0);
        // A trailing newline puts the caret on a new empty line.
        assert_eq!(text.caret("ab\n", &st, None, 3), [0.0, line]);
        // Wrapped text: the caret at the end is below the caret at the start.
        let words = "one two three four five six seven eight nine ten";
        let [full, _] = text.measure(words, &st, None);
        let end = text.caret(words, &st, Some(full / 3.0), words.len());
        assert!(end[1] >= 2.0 * line, "caret at the end of wrapped text is on line 3 or later: {end:?}");
    }

    #[test]
    fn hit_testing_inverts_the_caret_position() {
        let mut text = TextSystem::new();
        let st = style(13.0);
        let words = "alpha beta\ngamma delta epsilon";
        for width in [None, Some(60.0)] {
            for offset in [0, 3, 6, 11, 14, 20, 25, words.len()] {
                let [x, y] = text.caret(words, &st, width, offset);
                let back = text.hit(words, &st, width, [x + 0.5, y + 4.0]);
                assert_eq!(back, offset, "width {width:?}, offset {offset}");
            }
        }
        // Past the end of the text, and above it.
        assert_eq!(text.hit(words, &st, None, [5000.0, 5000.0]), words.len());
        assert_eq!(text.hit(words, &st, None, [0.0, -20.0]), 0);
    }

    #[test]
    fn selections_cover_each_line_they_touch() {
        let mut text = TextSystem::new();
        let st = style(13.0);
        let line = 13.0 * 1.25;
        let rects = text.selection_rects("one\ntwo\nthree", &st, None, 1, 10);
        assert_eq!(rects.len(), 3, "{rects:?}");
        assert_eq!(rects.iter().map(|r| r[1]).collect::<Vec<_>>(), [0.0, line, 2.0 * line]);
        assert!(rects[0][0] > 0.0, "the first line starts after its first letter");
        assert_eq!(rects[1][0], 0.0, "the middle line is selected from its start");
        assert!(text.selection_rects("one", &st, None, 2, 2).is_empty());
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

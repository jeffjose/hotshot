//! A frame as it was drawn last, kept to be shown again; and, when only
//! a few of its layers have changed since, as they do while one is moved
//! or typed into, drawn again only where those were and are, rather than
//! all of it.

use super::*;

/// A frame as drawn last at the size it is shown: at which change to it,
/// at which zoom, which part of it, its pixels, and how far each of its
/// layers reaches over it, at that zoom, for the frame worked in.
pub(super) struct Drawn {
    pub stamp: u64,
    pub zoom: f32,
    pub part: Rect,
    pub pixels: Vec<u32>,
    pub reaches: Vec<Rect>,
    /// Which of its layers were drawn, from one up to before another: all
    /// of a frame's, or the page's own between two frames.
    pub range: (usize, usize),
}

/// All of the layers of a frame, as a range of them.
pub(super) const ALL: (usize, usize) = (0, usize::MAX);

/// The smallest area over both.
pub(super) fn both(a: Rect, b: Rect) -> Rect {
    match (a.is_empty(), b.is_empty()) {
        (true, _) => b,
        (_, true) => a,
        _ => {
            let (x, y) = (a.x.min(b.x), a.y.min(b.y));
            Rect::new(x, y, a.right().max(b.right()) - x, a.bottom().max(b.bottom()) - y)
        }
    }
}

/// How far a layer drawn on its frame at `times` its size can reach,
/// shadow and all, in pixels of that, from its corner: no further than
/// it is drawn to, and a little more for good measure.
pub(super) fn reach(layer: &Layer, times: f32, text: &mut Text) -> Rect {
    match &layer.kind {
        Kind::Picture(picture) => {
            let mut at = picture.clone();
            (at.cx, at.cy, at.w, at.h) = (picture.cx * times, picture.cy * times, picture.w * times, picture.h * times);
            let lift = times * SHADOW_SIZE.max(1.0);
            let preset = Shadow::of_preset(picture.shadow.saturating_sub(1));
            let custom = [picture.drop];
            let layers = match picture.shadow {
                0 => &[][..],
                CUSTOM_SHADOW => &custom[..],
                _ => preset.layers(),
            };
            let out = layers.iter().map(|layer| (f32::from(layer.blur.max(0)) * 1.5 + f32::from(layer.spread).max(0.0) + f32::from(layer.x.abs().max(layer.y.abs()))) * lift).fold(0.0f32, f32::max);
            let corners = at.corners();
            let (left, top) = (corners.iter().map(|c| c.0).fold(f32::MAX, f32::min), corners.iter().map(|c| c.1).fold(f32::MAX, f32::min));
            let (right, bottom) = (corners.iter().map(|c| c.0).fold(f32::MIN, f32::max), corners.iter().map(|c| c.1).fold(f32::MIN, f32::max));
            Rect::new((left - out).floor() as i32 - 4, (top - out).floor() as i32 - 4, (right - left + 2.0 * out).ceil() as i32 + 9, (bottom - top + 2.0 * out).ceil() as i32 + 9)
        }
        Kind::Shape(shape) => {
            let shape = if times == 1.0 { shape.clone() } else { scaled(shape, times) };
            let body = shape.bounds(text).inflate((shape.stroke * 3.0) as i32 + 10);
            match shape.shadow {
                Some(drop) => {
                    let out = (f32::from(drop.blur.max(0)) * 1.5 + f32::from(drop.spread).max(0.0)).ceil() as i32 + 4;
                    both(body, body.inflate(out).moved(i32::from(drop.x), i32::from(drop.y)))
                }
                None => body,
            }
        }
    }
}

/// Whether a layer takes from what is under it beyond where it is drawn,
/// so that drawing a part of the frame alone would not draw it as it is:
/// a blur, a spotlight, which darkens all the rest, or a zoom.
fn reaches_beyond(layer: &Layer) -> bool {
    matches!(&layer.kind, Kind::Shape(shape) if matches!(shape.tool, Tool::Blur | Tool::Spotlight | Tool::Zoom))
}

impl Studio {
    /// Something has changed, but of the frame worked in only these
    /// layers, and what goes with them, as the lines stuck to them do:
    /// where those were and are is all of it drawn again.
    pub(super) fn changed_layers(&mut self, layers: &[usize]) {
        let before = self.doc.stamp;
        let (from, mut list) = match self.touched.take() {
            Some((from, upto, list)) if upto == before => (from, list),
            _ => (before, Vec::new()),
        };
        list.extend_from_slice(layers);
        list.sort_unstable();
        list.dedup();
        self.changed();
        // A stack laid out again moved more than these: all of it is drawn.
        if self.stacks_moved {
            return;
        }
        self.touched = Some((from, self.doc.stamp, list));
    }

    /// The frame worked in drawn again only where the layers changed since
    /// it was last drawn were and are, if that is all that has changed and
    /// what was drawn is at the zoom it is shown at; whether it could be.
    /// Changed are those it was told of, any that has moved or grown since,
    /// as a box holding another does round it, and the lines stuck to any
    /// of those; not every line stuck to something, which on a board is
    /// most of it. Where what is seen of it has gone past what was drawn,
    /// as a box dragged out past the rest does, what was drawn is grown to
    /// it, only what is new of it drawn.
    pub(super) fn redraw_touched(&mut self, index: usize, band: usize, range: (usize, usize), seen: Rect, whole: Rect) -> bool {
        let Some((from, upto, list)) = &self.touched else { return false };
        let Some(drawn) = self.drawn_of(index, band) else { return false };
        let fits = index == self.active
            && (range == ALL || range == (0, self.doc.layers.len()))
            && drawn.range == range
            && drawn.stamp == *from
            && self.doc.stamp == *upto
            && same_zoom(drawn.zoom, self.zoom)
            && drawn.reaches.len() == self.doc.layers.len()
            && list.iter().all(|&layer| layer < self.doc.layers.len())
            && !self.doc.layers.iter().any(reaches_beyond);
        if !fits {
            return false;
        }
        let (list, part, zoom) = (list.clone(), drawn.part, drawn.zoom);
        // Grown to what is seen and some way past it, past the edge of what
        // is drawn too, for it not to be grown again with each step of a
        // drag; so long as that leaves it not much larger than drawing it
        // afresh would.
        let grown = match part.intersect(seen) == seen {
            true => part,
            false => {
                let afresh = seen.inflate(ROOM).intersect(whole.inflate(ROOM));
                let grown = both(part, afresh);
                if i64::from(grown.w) * i64::from(grown.h) > 2 * i64::from(afresh.w) * i64::from(afresh.h) {
                    return false;
                }
                grown
            }
        };
        let now = self.reaches(zoom);
        let Some(drawn) = self.drawn_of(index, band) else { return false };
        let mut changed: Vec<bool> = (0..now.len()).map(|layer| now[layer] != drawn.reaches[layer]).collect();
        for &layer in &list {
            changed[layer] = true;
        }
        // The lines stuck to what changed, and to those, in turn.
        loop {
            let ids: Vec<u32> = self.doc.layers.iter().zip(&changed).filter(|(_, &changed)| changed).map(|(layer, _)| super::stacks::layer_id(layer)).collect();
            let mut more = false;
            for (layer, changed) in self.doc.layers.iter().zip(changed.iter_mut()) {
                if let Kind::Shape(shape) = &layer.kind {
                    if !*changed && shape.stuck.iter().flatten().any(|(id, ..)| ids.contains(id)) {
                        (*changed, more) = (true, true);
                    }
                }
            }
            if !more {
                break;
            }
        }
        // Where each changed one was and is, those that overlap as one.
        let mut damage: Vec<Rect> = Vec::new();
        for layer in (0..now.len()).filter(|&layer| changed[layer]) {
            let mut area = both(now[layer], drawn.reaches[layer]).intersect(grown);
            if area.is_empty() {
                continue;
            }
            while let Some(at) = damage.iter().position(|other| !other.intersect(area.inflate(1)).is_empty()) {
                area = both(area, damage.swap_remove(at));
            }
            damage.push(area);
        }
        // What is new of it where it has grown: above and below what was
        // drawn, and to either side of it.
        if grown != part {
            let rows = part.intersect(grown);
            for strip in [
                Rect::new(grown.x, grown.y, grown.w, rows.y - grown.y),
                Rect::new(grown.x, rows.bottom(), grown.w, grown.bottom() - rows.bottom()),
                Rect::new(grown.x, rows.y, rows.x - grown.x, rows.h),
                Rect::new(rows.right(), rows.y, grown.right() - rows.right(), rows.h),
            ] {
                if strip.w > 0 && strip.h > 0 {
                    damage.push(strip);
                }
            }
            let Some(drawn) = self.drawn_slot(index, band).as_mut() else { return false };
            let mut pixels = vec![0u32; (grown.w * grown.h) as usize];
            for row in 0..part.h {
                let into = ((part.y - grown.y + row) * grown.w + part.x - grown.x) as usize;
                pixels[into..into + part.w as usize].copy_from_slice(&drawn.pixels[(row * part.w) as usize..((row + 1) * part.w) as usize]);
            }
            (drawn.pixels, drawn.part) = (pixels, grown);
        }
        for area in damage {
            let fresh = self.doc.render_part(zoom, area, &mut self.text);
            let Some(drawn) = self.drawn_slot(index, band).as_mut() else { return false };
            for row in 0..area.h {
                let into = ((area.y - grown.y + row) * grown.w + area.x - grown.x) as usize;
                drawn.pixels[into..into + area.w as usize].copy_from_slice(&fresh[(row * area.w) as usize..((row + 1) * area.w) as usize]);
            }
        }
        let stamp = self.doc.stamp;
        let Some(drawn) = self.drawn_slot(index, band).as_mut() else { return false };
        (drawn.reaches, drawn.stamp) = (now, stamp);
        // Drawn as it is now: the next change counts from here.
        self.touched = None;
        true
    }

    /// How far each layer of the frame worked in reaches at a zoom, kept
    /// with what is drawn of it for the next change to draw no more than
    /// it needs.
    pub(super) fn reaches(&mut self, times: f32) -> Vec<Rect> {
        let layers = std::mem::take(&mut self.doc.layers);
        let reaches = layers.iter().map(|layer| reach(layer, times, &mut self.text)).collect();
        self.doc.layers = layers;
        reaches
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What is kept of the frame worked in, against the same part of it
    /// drawn whole: the same, to a shade where an edge falls.
    fn as_if_whole(studio: &mut Studio) {
        let drawn = studio.shown[studio.active].as_ref().unwrap();
        let (part, zoom, kept) = (drawn.part, drawn.zoom, drawn.pixels.clone());
        assert_eq!(drawn.stamp, studio.doc.stamp);
        let whole = studio.doc.render_part(zoom, part, &mut studio.text);
        let off = kept.iter().zip(&whole).filter(|(a, b)| [0, 8, 16, 24].iter().any(|shift| ((**a >> shift & 0xFF) as i32 - (**b >> shift & 0xFF) as i32).abs() > 2)).count();
        assert!(off == 0, "{off} pixels differ");
    }

    #[test]
    fn a_layer_moved_or_typed_into_is_drawn_again_only_where_it_was_and_is() {
        let mut studio = Studio::new(Doc::new(800, 600), Text::load(), 1400, 900, std::env::temp_dir(), "test-%Y.png".into());
        studio.add_picture(Image::new(300, 200, vec![0xFF33_8844; 300 * 200]), "one");
        if let Kind::Picture(picture) = &mut studio.doc.layers[0].kind {
            picture.shadow = 2;
        }
        let mut boxed = Shape::new(Tool::Rect, 0xE5484D, 3.5, (100.0, 100.0));
        boxed.points[1] = (220.0, 200.0);
        boxed.shadow = Some(frame::Layer { x: 0, y: 12, blur: 16, spread: 0, color: 0x6000_0000 });
        let mut words = Shape::new(Tool::Text, 0x1A1A1A, 2.0, (500.0, 450.0));
        words.text = "Hi".into();
        studio.doc.layers.push(Layer::shape(boxed));
        studio.doc.layers.push(Layer::shape(words));
        studio.changed();
        let mut out = vec![0u32; 1400 * 900];
        studio.draw(&mut out);
        // The box moved by a drag, over the picture and off it.
        studio.selected = vec![1];
        let start = studio.to_window((160.0, 150.0));
        studio.pointer_moved(start.0, start.1, Mods::default());
        studio.pressed(1, 1000);
        for step in 1..8 {
            studio.pointer_moved(start.0 + step * 25, start.1 + step * 9, Mods::default());
            assert!(studio.touched.is_some());
            studio.draw(&mut out);
            as_if_whole(&mut studio);
        }
        studio.released();
        // Typed into, and a letter taken back.
        studio.selected = vec![2];
        studio.edit_text(2);
        for key in [Key::Char('W'), Key::Char('a'), Key::Enter, Key::Char('y'), Key::Backspace] {
            studio.key(key, Mods::default(), true);
            assert!(studio.touched.is_some());
            studio.draw(&mut out);
            as_if_whole(&mut studio);
        }
        // Anything else changed draws all of it again.
        studio.key(Key::Escape, Mods::default(), true);
        assert!(studio.touched.is_none());
        studio.draw(&mut out);
        as_if_whole(&mut studio);
    }

    /// What is kept of a board's own layers, against the same part of
    /// them drawn whole, each as it is shown on the board's paper: the
    /// same, to a shade. A line drawn a shade into the paper here and not
    /// there leaves it opaque in one and clear in the other, which on the
    /// paper look the same.
    fn board_as_if_whole(studio: &mut Studio) {
        let drawn = studio.shown_page[0].as_ref().unwrap();
        let (part, zoom, range, kept) = (drawn.part, drawn.zoom, drawn.range, drawn.pixels.clone());
        assert_eq!(drawn.stamp, studio.doc.stamp);
        let whole = studio.doc.render_range(range, zoom, part, &mut studio.text);
        let paper = studio.paper().0;
        let shown = |color: u32| match color >> 24 {
            255 => color & 0xFF_FFFF,
            alpha => canvas::mix(paper, color & 0xFF_FFFF, alpha + (alpha >> 7)),
        };
        let off: Vec<(i32, i32, u32, u32)> = kept.iter().zip(&whole).enumerate().filter(|(_, (a, b))| [0, 8, 16].iter().any(|shift| ((shown(**a) >> shift & 0xFF) as i32 - (shown(**b) >> shift & 0xFF) as i32).abs() > 2)).map(|(at, (a, b))| (part.x + at as i32 % part.w, part.y + at as i32 / part.w, *a, *b)).collect();
        assert!(off.is_empty(), "{} pixels differ in {part:?}: {:x?}", off.len(), &off[..off.len().min(8)]);
    }

    #[test]
    fn a_box_dragged_on_a_board_is_drawn_again_only_where_it_and_what_goes_with_it_were_and_are() {
        use crate::studio::board::tests::{board, press_key, pull, type_in};
        let none = Mods::default();
        let mut studio = board();
        // A box with one in it, joined by an arrow to another, and an
        // arrow tied to nothing.
        press_key(&mut studio, Key::Char('r'), none);
        pull(&mut studio, (100.0, 100.0), (500.0, 400.0), none);
        press_key(&mut studio, Key::Enter, none);
        pull(&mut studio, (150.0, 150.0), (260.0, 210.0), none);
        type_in(&mut studio, "In");
        press_key(&mut studio, Key::Enter, none);
        pull(&mut studio, (700.0, 100.0), (860.0, 172.0), none);
        type_in(&mut studio, "Out");
        press_key(&mut studio, Key::Enter, none);
        press_key(&mut studio, Key::Escape, none);
        press_key(&mut studio, Key::Char('a'), none);
        pull(&mut studio, (268.0, 180.0), (692.0, 136.0), none);
        press_key(&mut studio, Key::Escape, none);
        press_key(&mut studio, Key::Char('a'), none);
        pull(&mut studio, (900.0, 500.0), (1000.0, 600.0), none);
        press_key(&mut studio, Key::Escape, none);
        press_key(&mut studio, Key::Escape, none);
        studio.selected.clear();
        let mut out = vec![0u32; (studio.width * studio.height) as usize];
        studio.draw(&mut out);
        board_as_if_whole(&mut studio);
        // The box inside dragged to the edge of the one round it, the one
        // the arrow goes to dragged down past all the rest, and the box
        // round the first dragged with it in it.
        let mut partly = 0;
        for (at, by) in [((200.0, 160.0), (30, 4)), ((780.0, 110.0), (6, 40)), ((300.0, 380.0), (-12, 15))] {
            let start = studio.to_window_page(at);
            studio.pointer_moved(start.0, start.1, none);
            studio.last_press.0 = 0;
            studio.pressed(1, 100_000);
            for step in 1..12 {
                studio.pointer_moved(start.0 + step * by.0, start.1 + step * by.1, none);
                partly += usize::from(studio.touched.is_some());
                studio.draw(&mut out);
                board_as_if_whole(&mut studio);
            }
            studio.released();
            studio.draw(&mut out);
            board_as_if_whole(&mut studio);
        }
        assert!(partly > 20, "drawn again only in part {partly} times");
    }
}

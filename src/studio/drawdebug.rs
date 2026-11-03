//! What drawing the canvas did, to look into when a board draws badly or
//! slowly. F12 steps through what is shown over the window: nothing; a
//! panel of what the frame took, part by part; that and an edge round
//! each part of the window just drawn again, a color for each way it was
//! drawn; and that and each shape's box and the points its hand-drawn
//! strokes go through. While any of it is shown, and with `--debug`, each
//! frame is written as a line at the end of `hotshot-draw.log` in the
//! folder for passing files, with what is pressed and typed between.

use super::*;
use std::collections::VecDeque;
use std::time::Instant;

/// How a part of the window came to be drawn.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Way {
    /// Drawn afresh at the zoom it is at.
    Whole,
    /// Drawn afresh at half the zoom, for while the page moves.
    Preview,
    /// Drawn afresh where only the view had changed, sharp once still.
    Sharp,
    /// Drawn again only where what changed was and is.
    Damage,
}

impl Way {
    fn name(self) -> &'static str {
        match self {
            Way::Whole => "whole",
            Way::Preview => "preview",
            Way::Sharp => "sharp",
            Way::Damage => "damage",
        }
    }

    /// The color an edge round it is drawn in.
    fn color(self) -> u32 {
        match self {
            Way::Whole => 0xFF9F1C,
            Way::Preview => 0xFFE14D,
            Way::Sharp => 0x2EC4B6,
            Way::Damage => 0xFF3B30,
        }
    }
}

/// A part of the window drawn in a frame: how, of which frame or band,
/// where in the window, and how long it took.
#[derive(Clone, Copy, Debug)]
pub(super) struct Piece {
    pub way: Way,
    pub index: usize,
    pub band: usize,
    /// Where, from the corner of the frame or band at the zoom it was
    /// drawn at, until it is placed in the window.
    pub rect: Rect,
    pub times: f32,
    pub placed: bool,
    pub ms: f32,
}

#[derive(Default)]
pub(super) struct DrawDebug {
    /// What is shown over the window: 0 nothing, 1 the panel, 2 and the
    /// parts drawn, 3 and each shape's box and its strokes' points.
    pub level: u8,
    pub pieces: Vec<Piece>,
    /// How long each part of this frame took, in order: the paper, the
    /// layers put on the window, what is drawn over them; and when the
    /// last of them ended.
    pub phases: Vec<(&'static str, f32)>,
    pub since: Option<Instant>,
    /// The paper's time, this frame.
    pub paper_ms: f32,
    /// When each of the last frames began, and how long it took, for
    /// how many a second.
    frames: VecDeque<(Instant, f32)>,
    /// What this frame says, shown over the window as it ends.
    said: Vec<String>,
    count: u64,
    /// The parts drawn last, kept on screen a moment as they fade.
    last: Vec<(Piece, Instant)>,
}

/// The file each frame is written to.
pub(super) fn log_path() -> std::path::PathBuf {
    std::env::temp_dir().join("hotshot-draw.log")
}

impl DrawDebug {
    /// Whether frames are written down.
    pub fn logging(&self) -> bool {
        self.level > 0 || crate::debugging()
    }

    /// A part drawn, at a time `started`.
    pub fn piece(&mut self, way: Way, index: usize, band: usize, rect: Rect, times: f32, started: Instant) {
        if self.logging() {
            self.pieces.push(Piece { way, index, band, rect, times, placed: false, ms: started.elapsed().as_secs_f32() * 1000.0 });
        }
    }

    /// The parts of a frame or band just drawn placed in the window, by
    /// where its corner is and the zoom it is shown at.
    pub fn place(&mut self, index: usize, band: usize, origin: (i32, i32), zoom: f32) {
        for piece in self.pieces.iter_mut().filter(|piece| !piece.placed && piece.index == index && piece.band == band) {
            let by = zoom / piece.times.max(1e-6);
            let r = piece.rect;
            let (left, top) = ((r.x as f32 * by).floor() as i32, (r.y as f32 * by).floor() as i32);
            let (right, bottom) = ((r.right() as f32 * by).ceil() as i32, (r.bottom() as f32 * by).ceil() as i32);
            piece.rect = Rect::new(origin.0 + left, origin.1 + top, right - left, bottom - top);
            piece.placed = true;
        }
    }
}

impl Studio {
    /// F12: the next of what drawing debug shows, round to nothing.
    pub(super) fn step_draw_debug(&mut self) {
        let debug = &mut self.draw_debug;
        debug.level = (debug.level + 1) % 4;
        let says = match debug.level {
            0 => "Drawing debug off".to_owned(),
            1 => format!("Drawing debug: times, written to {}", log_path().display()),
            2 => "Drawing debug: times and the parts drawn again".to_owned(),
            _ => "Drawing debug: times, parts, and each shape's box and points".to_owned(),
        };
        crate::log_line(format_args!("== {says}"));
        self.say(says);
        self.dirty = true;
    }

    /// A frame done: written down, and what it took shown over it.
    pub(super) fn end_draw_debug(&mut self, canvas: &mut Canvas, started: Instant) {
        let total = started.elapsed().as_secs_f32() * 1000.0;
        let (masks, mask_ms, mask_px) = shapes::MASKS_MADE.take();
        let (fills, fill_ms, fill_px) = shapes::FILLS_MADE.take();
        if !self.draw_debug.logging() {
            self.draw_debug.pieces.clear();
            return;
        }
        let now = Instant::now();
        let debug = &mut self.draw_debug;
        debug.count += 1;
        debug.frames.push_back((now, total));
        while debug.frames.front().is_some_and(|(when, _)| now.duration_since(*when).as_secs_f32() > 1.0) {
            debug.frames.pop_front();
        }
        let pieces = std::mem::take(&mut debug.pieces);
        let drawn_ms: f32 = pieces.iter().map(|piece| piece.ms).sum::<f32>().max(0.0);
        let kinds = |way: Way| pieces.iter().filter(|piece| piece.way == way).count();
        let area: i64 = pieces.iter().map(|piece| i64::from(piece.rect.w) * i64::from(piece.rect.h)).sum();
        let layers = self.doc.layers.len();
        let state = match (self.moving.is_some(), self.drag != Drag::None) {
            (true, _) => "moving",
            (_, true) => "dragging",
            _ => "still",
        };
        let phases: String = debug.phases.iter().map(|(name, ms)| format!("{name} {ms:.1}")).collect::<Vec<_>>().join(" · ");
        let parts: Vec<String> = pieces.iter().map(|piece| format!("{} {}x{}@{},{} z{:.2} {:.1}ms", piece.way.name(), piece.rect.w, piece.rect.h, piece.rect.x, piece.rect.y, piece.times, piece.ms)).collect();
        crate::log_line(format_args!(
            "#{} {total:.1}ms {state} zoom {:.3} layers {layers} | {} | drawn {} parts {:.1}ms {:.0}k px [{}] | masks {masks} {mask_ms:.1}ms {:.0}k px, fills {fills} {fill_ms:.1}ms {:.0}k px",
            debug.count,
            self.zoom,
            phases,
            pieces.len(),
            drawn_ms,
            area as f32 / 1000.0,
            parts.join(", "),
            mask_px as f32 / 1000.0,
            fill_px as f32 / 1000.0,
        ));
        let fps = debug.frames.len();
        let worst = debug.frames.iter().map(|(_, ms)| *ms).fold(0.0, f32::max);
        debug.said = vec![
            format!("frame {total:.1} ms · {fps}/s, worst {worst:.1} · {state}"),
            format!("zoom {:.0}% · {layers} layers", self.zoom * 100.0),
            phases.clone(),
            format!("drawn {} ms: {} whole, {} preview, {} sharp, {} damage · {:.0}k px", format_args!("{drawn_ms:.1}"), kinds(Way::Whole), kinds(Way::Preview), kinds(Way::Sharp), kinds(Way::Damage), area as f32 / 1000.0),
            format!("lines worked out {masks} · {mask_ms:.1} ms · {:.0}k px", mask_px as f32 / 1000.0),
            format!("fills worked out {fills} · {fill_ms:.1} ms · {:.0}k px", fill_px as f32 / 1000.0),
            format!("F12 · {}", ["", "times", "+ parts", "+ shapes"][usize::from(debug.level)]),
        ];
        debug.last.retain(|(_, when)| now.duration_since(*when).as_millis() < 600);
        debug.last.extend(pieces.into_iter().map(|piece| (piece, now)));
        if debug.level == 0 {
            return;
        }
        // The shapes, their boxes and the points their strokes go through.
        if debug.level >= 3 && self.on_board() {
            for layer in &self.doc.layers {
                let (x, y, w, h) = layer.bounds(&mut self.text);
                let a = self.to_window_page((x, y));
                let b = self.to_window_page((x + w, y + h));
                canvas.round_outline(a.0, a.1, (b.0 - a.0).max(1), (b.1 - a.1).max(1), 0, 1, 0x00B4D8);
                let Kind::Shape(shape) = &layer.kind else { continue };
                if !shape.sketch {
                    continue;
                }
                for ink in shape.sketch_strokes() {
                    for (at, &point) in ink.points.iter().enumerate() {
                        let p = self.to_window_page(point);
                        let color = if at == 0 { 0x2F9E44 } else { 0xD6336C };
                        canvas.fill(p.0 - 1, p.1 - 1, 2, 2, color);
                    }
                }
            }
        }
        // The parts drawn again, fading over a moment.
        if self.draw_debug.level >= 2 {
            for (piece, when) in &self.draw_debug.last {
                let left = 256u32.saturating_sub(now.duration_since(*when).as_millis() as u32 * 256 / 600);
                let r = piece.rect;
                canvas.round_fill(r.x, r.y, r.w, 1, 0, piece.way.color(), left);
                canvas.round_fill(r.x, r.bottom() - 1, r.w, 1, 0, piece.way.color(), left);
                canvas.round_fill(r.x, r.y, 1, r.h, 0, piece.way.color(), left);
                canvas.round_fill(r.right() - 1, r.y, 1, r.h, 0, piece.way.color(), left);
            }
            if !self.draw_debug.last.is_empty() {
                self.dirty = true;
            }
        }
        // The panel, under the switch at the top left.
        let lines = self.draw_debug.said.clone();
        let high = Text::line_height(12) + 3;
        let wide = lines.iter().map(|line| self.text.width(line, 12)).max().unwrap_or(0) + 20;
        let (x, y) = (INSET + 12, INSET + 56);
        canvas.round_fill(x, y, wide, high * lines.len() as i32 + 14, 8, 0x101010, 225);
        for (at, line) in lines.iter().enumerate() {
            self.text.draw(canvas, x + 10, y + 7 + at as i32 * high, 12, if at == 0 { 0xFFFFFF } else { 0xC8C8C8 }, line);
        }
        if self.draw_debug.level >= 2 {
            let mut along = x;
            for way in [Way::Whole, Way::Preview, Way::Sharp, Way::Damage] {
                canvas.round_fill(along, y + high * lines.len() as i32 + 20, 10, 10, 2, way.color(), 256);
                self.text.draw(canvas, along + 14, y + high * lines.len() as i32 + 18, 11, 0x303030, way.name());
                along += 14 + self.text.width(way.name(), 11) + 12;
            }
        }
    }
}

impl DrawDebug {
    /// A frame begun.
    pub fn begin(&mut self, now: Instant) {
        self.phases.clear();
        self.since = Some(now);
    }

    /// A part of the frame done, called so.
    pub fn mark(&mut self, name: &'static str) {
        if let Some(since) = self.since {
            self.phases.push((name, since.elapsed().as_secs_f32() * 1000.0));
            self.since = Some(Instant::now());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::studio::board::tests::{board, press_key, pull};

    #[test]
    fn f12_steps_through_what_drawing_debug_shows_and_each_frame_says_its_parts() {
        let mut studio = board();
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (100.0, 100.0), (260.0, 172.0), Mods::default());
        press_key(&mut studio, Key::Escape, Mods::default());
        let mut out = vec![0u32; (studio.width * studio.height) as usize];
        for level in [1, 2, 3, 0] {
            studio.key(Key::F12, Mods::default(), true);
            assert_eq!(studio.draw_debug.level, level);
            studio.changed();
            studio.draw(&mut out);
            assert!(studio.draw_debug.pieces.is_empty(), "said and let go each frame");
            if level > 0 {
                assert!(studio.draw_debug.said.iter().any(|line| line.starts_with("frame ")));
                assert!(studio.draw_debug.phases.iter().any(|(name, _)| *name == "layers"));
            }
        }
    }
}

//! How long the canvas takes to draw a frame of the window, for each kind
//! of thing done in it, on a large window full of layers: two large
//! pictures, two hundred shapes and text. Run by hand, with
//! `cargo test --release draw_timings -- --ignored --nocapture`.

use super::*;
use std::time::{Duration, Instant};

/// A picture with something in it all over, as a photograph has.
fn photo(width: i32, height: i32, seed: u32) -> Image {
    Image::new(width, height, (0..width * height).map(|at| {
        let (x, y) = ((at % width) as u32, (at / width) as u32);
        let wave = (x.wrapping_mul(7) ^ y.wrapping_mul(13)).wrapping_add(seed) & 0xFF;
        0xFF00_0000 | (x * 255 / width as u32) << 16 | (y * 255 / height as u32) << 8 | wave
    }).collect())
}

fn full(width: i32, height: i32) -> Studio {
    let mut studio = Studio::new(Doc::new(2400, 1600), Text::load(), width, height, std::env::temp_dir(), "perf-%Y.png".into());
    studio.add_picture(photo(2000, 1300, 1), "one");
    studio.add_picture(photo(1600, 1000, 2), "two");
    for at in 0..200 {
        let (x, y) = (60.0 + (at % 20) as f32 * 115.0, 60.0 + (at / 20) as f32 * 150.0);
        let tool = [Tool::Rect, Tool::Ellipse, Tool::Arrow, Tool::Rect, Tool::Text][at % 5];
        let mut shape = Shape::new(tool, 0xE5484D + at as u32 * 97, 3.5, (x, y));
        match tool {
            Tool::Text => shape.text = format!("Label {at}"),
            _ => shape.points[1] = (x + 90.0, y + 110.0),
        }
        if at % 3 == 0 && tool == Tool::Rect {
            (shape.filled, shape.fill) = (true, 0x3E63DD);
        }
        studio.doc.layers.push(Layer::shape(shape));
    }
    studio.changed();
    studio
}

fn figures(name: &str, mut times: Vec<Duration>) {
    times.sort();
    let at = |part: usize| times[(times.len() * part / 100).min(times.len() - 1)];
    println!("  {name:<28} median {:>8.2?}   90% {:>8.2?}   most {:>8.2?}", at(50), at(90), times[times.len() - 1]);
}

/// Each step done, then the window drawn, timed together, as the canvas
/// does for each run of events.
fn timed(studio: &mut Studio, out: &mut [u32], steps: i32, mut step: impl FnMut(&mut Studio, i32)) -> Vec<Duration> {
    (0..steps)
        .map(|at| {
            let began = Instant::now();
            step(studio, at);
            if studio.take_dirty() {
                studio.draw(out);
            }
            began.elapsed()
        })
        .collect()
}

/// The same canvas written to a file, to open in hotshot itself:
/// `PERF_CANVAS=/some/where.hotshot cargo test --release write_full_canvas -- --ignored`.
#[test]
#[ignore]
fn write_full_canvas() {
    let Some(path) = std::env::var_os("PERF_CANVAS") else { return };
    let mut studio = full(1400, 900);
    studio.path = Some(PathBuf::from(path));
    studio.save(false);
}

/// How long drawing the frame of the full canvas takes at the size of a
/// 4K screen, all of it, and only its pictures or only its shapes.
#[test]
#[ignore]
fn render_timings() {
    let studio = full(3840, 2160);
    let mut text = Text::load();
    let times = 1.6;
    let part = Rect::new(0, 0, 3840, 2160);
    let pictures = Doc { layers: studio.doc.layers.iter().filter(|layer| matches!(layer.kind, Kind::Picture(_))).cloned().collect(), ..studio.doc.clone() };
    let shapes = Doc { layers: studio.doc.layers.iter().filter(|layer| matches!(layer.kind, Kind::Shape(_))).cloned().collect(), ..studio.doc.clone() };
    let empty = Doc { layers: Vec::new(), ..studio.doc.clone() };
    let only = |tool: Tool| Doc { layers: studio.doc.layers.iter().filter(|layer| matches!(&layer.kind, Kind::Shape(shape) if shape.tool == tool)).cloned().collect(), ..studio.doc.clone() };
    let (rects, ovals, arrows, words) = (only(Tool::Rect), only(Tool::Ellipse), only(Tool::Arrow), only(Tool::Text));
    for (name, doc) in [("everything", &studio.doc), ("pictures", &pictures), ("shapes", &shapes), ("80 boxes", &rects), ("40 ovals", &ovals), ("40 arrows", &arrows), ("40 texts", &words), ("nothing", &empty)] {
        figures(name, (0..5).map(|_| {
            let began = Instant::now();
            std::hint::black_box(doc.render_part(times, part, &mut text));
            began.elapsed()
        }).collect());
    }
}

#[test]
#[ignore]
fn draw_timings() {
    for (width, height) in [(3840, 2160), (1776, 1540)] {
        println!("{width}x{height}:");
        let mut studio = full(width, height);
        let mut out = vec![0u32; (width * height) as usize];
        studio.draw(&mut out);
        let none = Mods::default();
        let middle = (width / 2, height / 2);
        let empty = studio.to_window((2350.0, 1580.0));
        figures("first draw", timed(&mut studio, &mut out, 1, |studio, _| studio.dirty = true));
        figures("hover over the page", timed(&mut studio, &mut out, 60, |studio, at| studio.pointer_moved(empty.0 + at % 7, empty.1 + at % 5, none)));
        let shape = studio.to_window((60.0 + 45.0, 60.0 + 50.0));
        figures("hover over shapes", timed(&mut studio, &mut out, 60, |studio, at| studio.pointer_moved(shape.0 + (at % 9) * 12, shape.1, none)));
        figures("hover over the panel", timed(&mut studio, &mut out, 60, |studio, at| studio.pointer_moved(width - 150, 200 + at * 7, none)));
        studio.pointer_moved(shape.0, shape.1, none);
        studio.pressed(1, 1000);
        assert!(matches!(studio.drag, Drag::Move { .. }) && !studio.selected.is_empty());
        figures("drag a shape", timed(&mut studio, &mut out, 60, |studio, at| studio.pointer_moved(shape.0 + at * 3, shape.1 + at * 2, none)));
        studio.released();
        studio.pointer_moved(empty.0, empty.1, none);
        studio.pressed(1, 5000);
        figures("marquee", timed(&mut studio, &mut out, 60, |studio, at| studio.pointer_moved(empty.0 - at * 10, empty.1 - at * 6, none)));
        studio.released();
        let text = studio.doc.layers.iter().position(|layer| matches!(&layer.kind, Kind::Shape(shape) if shape.tool == Tool::Text)).unwrap_or(0);
        studio.selected = vec![text];
        studio.edit_text(text);
        figures("type text", timed(&mut studio, &mut out, 40, |studio, at| {
            studio.key(Key::Char((b'a' + (at % 26) as u8) as char), none, true);
        }));
        studio.key(Key::Escape, none, true);
        let ctrl = Mods { ctrl: true, ..none };
        studio.wheel_at(middle.0, middle.1, ctrl);
        figures("wheel zoom", timed(&mut studio, &mut out, 30, |studio, at| studio.scrolled(0.0, if at < 15 { -1.0 } else { 1.0 })));
        studio.moving = Some(Instant::now() - SETTLE);
        figures("sharp after zoom", timed(&mut studio, &mut out, 1, |studio, _| studio.settle()));
        studio.wheel_at(middle.0, middle.1, none);
        figures("wheel pan", timed(&mut studio, &mut out, 30, |studio, at| studio.scrolled(0.0, if at < 15 { 0.5 } else { -0.5 })));
    }
}

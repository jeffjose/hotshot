//! How a board looks, for the page as a whole: by hand with rounded corners
//! and looser lines, as it begins; by hand and angular; or clean, hairline
//! borders and Geist, as Linear's and Vercel's are. And what it is on:
//! light paper, dark gray or black, every color of the board turning to
//! its shade for the dark, as Radix's light and dark scales do, so nothing
//! is picked again. Switching draws it again; nothing moves.

use super::*;

/// The looks, and the grounds, by what each is called.
pub(in crate::studio) const LOOKS: [&str; 3] = ["Rounded", "Sketchy", "Clean"];
pub(in crate::studio) const GROUNDS: [&str; 3] = ["Light", "Dark gray", "Black"];

/// Each ground's paper and its dots.
pub(in crate::studio) const PAPERS: [(u32, u32); 3] = [(shapes::PAPER, 0xD8D6CF), (0x232323, 0x3C3C3C), (0x000000, 0x2A2A2A)];

/// A board's colors and their shades on the dark: its line colors, its
/// fills, a note's edge, white and the paper.
const DARK: [(u32, u32); 33] = [
    (0x1E1E1E, 0xE6E6E6),
    (0xE03131, 0xFF8787),
    (0x2F9E44, 0x8CE99A),
    (0x1971C2, 0x74C0FC),
    (0xF08C00, 0xFFC078),
    (0x868E96, 0xADB5BD),
    (0xFFF3BF, 0x4A3D06),
    (0xD3F9D8, 0x173D25),
    (0xD0EBFF, 0x10304F),
    (0xFFE3E3, 0x4A1B20),
    (0xE5DBFF, 0x2F2454),
    (0xFFFFFF, 0x2B2B2B),
    (0x000000, 0xE6E6E6),
    // The stronger colors a diagram tells its parts apart by, deeper;
    // its paler fills and grays, darker; its other lines, lighter.
    (0x74C0FC, 0x1C7ED6),
    (0xFFD43B, 0xE8A900),
    (0x8CE99A, 0x37B24D),
    (0xFFA8A8, 0xE03131),
    (0xB197FC, 0x7048E8),
    (0x66D9E8, 0x1098AD),
    (0xFFC078, 0xF76707),
    (0xFAA2C1, 0xD6336C),
    (0xC5F6FA, 0x0B3A40),
    (0xFFE8CC, 0x4A2A0A),
    (0xFFDEEB, 0x4A1830),
    (0xE7F5FF, 0x0C2742),
    (0xF3F9FF, 0x0A1D30),
    (0xF1F3F5, 0x2E2E2E),
    (0xE9ECEF, 0x3A3F44),
    (0xCED4DA, 0x495057),
    (0x7048E8, 0xB197FC),
    (0x0C8599, 0x66D9E8),
    (0xE8590C, 0xFFA94D),
    (0xC2255C, 0xF783AC),
];

/// A color as a ground shows it: on the dark, its shade there if it is one
/// of the board's.
pub(in crate::studio) fn shade(color: u32, ground: u8) -> u32 {
    match ground {
        0 => color,
        _ => DARK.iter().find(|(light, _)| *light == color & 0xFF_FFFF).map_or_else(|| {
            // Another color: a light one deepened, keeping a little of its
            // hue, a dark one lifted, as the shades above are.
            let part = |shift: u32| (color >> shift & 0xFF) as f32 / 255.0;
            let light = 0.2126 * part(16) + 0.7152 * part(8) + 0.0722 * part(0);
            match light {
                l if l > 0.6 => canvas::mix(0x1C1C1C, color, 46),
                l if l < 0.35 => canvas::mix(color, 0xFFFFFF, 190),
                _ => color,
            }
        }, |&(_, dark)| dark),
    }
}

/// The look and ground last chosen, kept in `board-look` among hotshot's
/// state, for a new board to begin as.
pub(in crate::studio) fn last_look() -> (u8, u8) {
    if cfg!(test) {
        return (0, 0);
    }
    let kept = crate::output::state_file("board-look").and_then(|file| std::fs::read_to_string(file).ok()).unwrap_or_default();
    let mut words = kept.split_whitespace().filter_map(|word| word.parse::<u8>().ok());
    (words.next().unwrap_or(0).min(2), words.next().unwrap_or(0).min(2))
}

fn keep_look(look: u8, ground: u8) {
    if cfg!(test) {
        return;
    }
    let Some(file) = crate::output::state_file("board-look") else { return };
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(file, format!("{look} {ground}\n"));
}

/// A page made a board for the first time: drawn as the last board was.
pub(in crate::studio) fn first_look(page: &mut Page) {
    if !page.styled {
        (page.look, page.ground) = last_look();
        page.styled = true;
    }
}

/// A shape as a board of a look, on a ground, draws it: drawn `times` its
/// size, as an export does.
pub(in crate::studio) fn dress(shape: &mut Shape, look: u8, ground: u8, times: f32) {
    let boxy = matches!(shape.tool, Tool::Rect | Tool::Ellipse);
    match look {
        0 => {
            shape.rounded = shape.sketch && shape.tool == Tool::Rect;
            if shape.rounded {
                let (_, _, w, h) = shape.frame();
                shape.radius = (w.min(h) * 0.3).min(7.0 * times).max(2.0);
                shape.corners = None;
            }
        }
        2 => {
            // Clean: no hand, a hairline, rounded a little, in Geist.
            if shape.sketch {
                shape.sketch = false;
                shape.stroke = 1.5 * times;
                if boxy {
                    shape.radius = 8.0 * times;
                }
            }
            if shape.face == LETTERING {
                shape.face = 0;
            }
        }
        _ => {}
    }
    if ground > 0 {
        shape.color = shade(shape.color, ground);
        shape.fill = shade(shape.fill, ground);
        // Black kept a hair off nought, which says the light paper.
        shape.paper = PAPERS[usize::from(ground.min(2))].0.max(1);
    }
    shape.touch();
}

impl Studio {
    /// The board's look and ground given what is drawn of it, for it to be
    /// drawn so; none off a board.
    pub(in crate::studio) fn sync_look(&mut self) {
        let page = &self.pages[self.page];
        // A board made a page of frames is drawn crisp, as Clean on light
        // is, its shapes left as they were for when it is a board again.
        let style = match (page.board, page.styled) {
            (true, _) => Some((page.look, page.ground, true)),
            (false, true) => Some((2, 0, false)),
            (false, false) => None,
        };
        // The frames of a board only what of it is exported, on its paper.
        let area = page.board.then(|| PAPERS[usize::from(page.ground.min(2))].0);
        let active = self.active;
        match active {
            LOOSE => self.doc.board = style,
            _ => {
                (self.doc.area, self.doc.board) = (area, None);
                self.pages[self.page].loose.board = style;
            }
        }
        for (index, frame) in self.pages[self.page].frames.iter_mut().enumerate() {
            if index != active {
                (frame.area, frame.board) = (area, None);
            }
        }
    }

    /// The board drawn another way, or on another ground: one step to undo.
    pub(in crate::studio) fn set_look(&mut self, look: Option<u8>, ground: Option<u8>) {
        self.checkpoint();
        let page = &mut self.pages[self.page];
        if let Some(look) = look {
            page.look = look.min(2);
        }
        if let Some(ground) = ground {
            page.ground = ground.min(2);
        }
        page.styled = true;
        let (look, ground) = (page.look, page.ground);
        keep_look(look, ground);
        self.shown.clear();
        self.shown_page.clear();
        self.changed();
        self.say(format!("{} on {}", LOOKS[usize::from(look)], GROUNDS[usize::from(ground)].to_lowercase()));
    }

    /// The paper of the board shown, and its dots.
    pub(in crate::studio) fn paper(&self) -> (u32, u32) {
        PAPERS[usize::from(self.pages[self.page].ground.min(2))]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::studio::board::tests::*;

    #[test]
    fn a_board_is_drawn_rounded_sketchy_or_clean_on_light_dark_or_black_and_nothing_moves() {
        let mut studio = board();
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (100.0, 100.0), (300.0, 200.0), Mods::default());
        type_in(&mut studio, "Cache");
        press_key(&mut studio, Key::Escape, Mods::default());
        studio.selected.clear();
        let was = shape(&studio, 0).clone();
        assert_eq!((studio.pages[0].look, studio.pages[0].ground), (0, 0), "rounded on light, to begin with");
        // Rounded: its corner is paper, as an angular one's is not; the
        // wobble the same every time, by what it is called.
        if let Kind::Shape(shape) = &mut studio.doc.layers[0].kind {
            shape.id = 7;
            shape.touch();
        }
        studio.changed();
        let paper =|studio: &mut Studio| (100..106).flat_map(|x| (100..106).map(move |y| (x as f32, y as f32))).filter(|&at| matches!(shown(studio, at), shapes::PAPER | DOTS)).count();
        let rounded = paper(&mut studio);
        studio.set_look(Some(1), None);
        assert!(rounded > paper(&mut studio) + 4, "more paper in a rounded corner");
        // Dark gray: its paper and its fill turn; its frame stays put.
        studio.set_look(None, Some(1));
        assert_eq!(shown(&mut studio, (200.0, 130.0)), shade(FILLS[2], 1));
        assert!(matches!(shown(&mut studio, (500.0, 500.0)), 0x232323 | 0x3C3C3C));
        studio.set_look(Some(2), Some(2));
        assert_eq!(shape(&studio, 0).frame(), was.frame());
        assert_eq!((shape(&studio, 0).sketch, shape(&studio, 0).face), (true, LETTERING), "only drawn otherwise");
        // Kept in the file, and exported on its ground.
        let back = read_pages(&write_pages(&studio.gathered())).unwrap();
        assert_eq!((back[0].look, back[0].ground), (2, 2));
        assert_eq!(studio.board_ground(), 0x000000);
        studio.undo();
        assert_eq!(studio.pages[0].ground, 1);
    }

    #[test]
    fn a_boards_look_stays_its_own_through_design_a_file_and_undo() {
        let mut studio = board();
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (100.0, 100.0), (300.0, 200.0), Mods::default());
        press_key(&mut studio, Key::Escape, Mods::default());
        studio.selected.clear();
        let was = shape(&studio, 0);
        studio.set_look(Some(2), Some(1));
        let clean = shown(&mut studio, (101.0, 150.0));
        // Design: drawn crisp, its shapes as they were.
        studio.set_mode(false);
        let kept = shape(&studio, 0);
        assert_eq!((kept.sketch, kept.rounded, kept.face), (was.sketch, was.rounded, was.face));
        assert_eq!((studio.pages[0].look, studio.pages[0].ground), (2, 1));
        // Kept in the file while it is a page of frames.
        let back = read_pages(&write_pages(&studio.gathered())).unwrap();
        assert!(!back[0].board && (back[0].look, back[0].ground) == (2, 1));
        // A board again: Clean on dark gray, as it was.
        studio.set_mode(true);
        assert_eq!((studio.pages[0].look, studio.pages[0].ground), (2, 1));
        assert_eq!(shown(&mut studio, (101.0, 150.0)), clean);
        studio.undo();
        studio.undo();
        assert!(studio.on_board() && studio.pages[0].look == 2);
        // Read back from a file and made a board, still its own.
        let mut back = read_pages(&write_pages(&studio.gathered())).unwrap();
        back[0].board = false;
        let back = read_pages(&write_pages(&back)).unwrap();
        assert_eq!((back[0].look, back[0].ground, back[0].styled), (2, 1, true));
    }

    #[test]
    fn a_boards_line_is_two_pixels_of_its_ink_on_the_screen_and_its_words_modest() {
        for look in [0, 1, 2] {
            let mut studio = board();
            press_key(&mut studio, Key::Char('r'), Mods::default());
            pull(&mut studio, (100.0, 100.0), (260.0, 172.0), Mods::default());
            type_in(&mut studio, "Cache");
            press_key(&mut studio, Key::Escape, Mods::default());
            studio.selected.clear();
            studio.set_look(Some(look), None);
            assert_eq!(shape(&studio, 0).font_px, 16, "lettered at a modest size");
            // Across its left side, half way down: a line about two pixels
            // wide, its edges blended with the paper, never darker than
            // its ink as edges over nothing once were.
            let row: Vec<u32> = (90..112).map(|x| shown(&mut studio, (x as f32, 136.0))).collect();
            let dark = row.iter().filter(|&&pixel| (pixel & 0xFF) < 0x80).count();
            assert!((1..=4).contains(&dark), "{}: {dark} dark pixels across its line", LOOKS[usize::from(look)]);
            assert!(row.iter().all(|&pixel| (pixel & 0xFF) >= 0x1A), "{}: an edge darker than its ink", LOOKS[usize::from(look)]);
        }
    }

    #[test]
    fn a_hand_drawn_box_is_gone_round_twice_the_second_time_fainter_with_no_ticks_past_its_corners() {
        let mut studio = board();
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (100.0, 100.0), (260.0, 172.0), Mods::default());
        press_key(&mut studio, Key::Escape, Mods::default());
        for rounded in [false, true] {
            let mut box_ = shape(&studio, 0);
            box_.rounded = rounded;
            let strokes = box_.sketch_strokes();
            assert_eq!(strokes.len(), 2, "two passes, each one closed run");
            assert!(strokes[0].2 > strokes[1].2 && strokes[0].1 > strokes[1].1, "the second thinner and fainter");
            // Nowhere more than a few pixels off the box: no tick marks
            // past its corners.
            for (points, ..) in &strokes {
                assert!(points.iter().all(|&(x, y)| (97.0..=263.0).contains(&x) && (97.0..=175.0).contains(&y)));
                assert_eq!(points.first(), points.last(), "closed");
            }
        }
    }

    /// A board of every kind of shape, with a Mermaid flowchart, written
    /// as a PNG into `$HOTSHOT_SNAPSHOTS` in each look on each ground, at
    /// `$HOTSHOT_TIMES` its size, or its own.
    ///
    ///     HOTSHOT_SNAPSHOTS=shots cargo test look_snapshots -- --ignored
    #[test]
    #[ignore]
    fn look_snapshots() {
        let Some(to) = std::env::var_os("HOTSHOT_SNAPSHOTS") else { return };
        let to = std::path::PathBuf::from(to);
        let times: f32 = std::env::var("HOTSHOT_TIMES").ok().and_then(|times| times.parse().ok()).unwrap_or(1.0);
        std::fs::create_dir_all(&to).unwrap();
        let mut studio = board();
        assert!(studio.bring_mermaid("flowchart LR\n  API --> Database\n  API --> Cache"));
        let boxes: Vec<(f32, f32, f32, f32)> = studio.doc.layers.iter().map(|layer| layer.bounds(&mut studio.text)).collect();
        let (x, y, _, _) = crate::studio::actions::union(&boxes).unwrap();
        for layer in &mut studio.doc.layers {
            layer.translate(40.0 - x, 40.0 - y);
        }
        let mut add = |studio: &mut Studio, tool: Tool, form: u8, from: (f32, f32), to: (f32, f32), words: &str| {
            let mut shape = Shape::new(tool, INK, LINE, from);
            shape.points[1] = to;
            studio.board_dress(&mut shape);
            if tool == Tool::Rect {
                shape.form = form;
            }
            if form == shapes::FORM_STICKY {
                (shape.color, shape.fill, shape.shadow) = (QUIET, FILLS[0], Some(frame::Layer { x: 0, y: 3, blur: 8, spread: 0, color: 0x2E00_0000 }));
            }
            shape.text = words.into();
            shape.touch();
            studio.doc.layers.push(Layer::shape(shape));
        };
        add(&mut studio, Tool::Ellipse, 0, (40.0, 200.0), (160.0, 256.0), "Oval");
        add(&mut studio, Tool::Rect, shapes::FORM_DIAMOND, (190.0, 190.0), (310.0, 266.0), "Ready?");
        add(&mut studio, Tool::Rect, shapes::FORM_CYLINDER, (340.0, 192.0), (440.0, 268.0), "Postgres");
        add(&mut studio, Tool::Rect, shapes::FORM_PERSON, (470.0, 186.0), (550.0, 276.0), "User");
        add(&mut studio, Tool::Rect, shapes::FORM_CLOUD, (580.0, 190.0), (720.0, 270.0), "Internet");
        add(&mut studio, Tool::Rect, shapes::FORM_STICKY, (40.0, 300.0), (170.0, 420.0), "Ship it Friday");
        add(&mut studio, Tool::Rect, shapes::FORM_HEXAGON, (200.0, 310.0), (320.0, 366.0), "Retry");
        add(&mut studio, Tool::Rect, shapes::FORM_STADIUM, (350.0, 310.0), (470.0, 366.0), "Start");
        add(&mut studio, Tool::Arrow, 0, (200.0, 400.0), (340.0, 420.0), "");
        add(&mut studio, Tool::Line, 0, (370.0, 400.0), (520.0, 400.0), "");
        let mut dashed = Shape::new(Tool::Arrow, INK, LINE, (550.0, 330.0));
        dashed.points[1] = (700.0, 360.0);
        studio.board_dress(&mut dashed);
        dashed.dashed = true;
        studio.doc.layers.push(Layer::shape(dashed));
        let mut pen = Shape::new(Tool::Pen, INK, LINE, (560.0, 420.0));
        pen.points = (0..=24).map(|step| {
            let t = step as f32 / 24.0;
            (560.0 + t * 150.0, 420.0 - (t * 9.0).sin() * 14.0 + t * 10.0)
        }).collect();
        studio.board_dress(&mut pen);
        studio.doc.layers.push(Layer::shape(pen));
        studio.selected.clear();
        for (look, name) in LOOKS.iter().enumerate() {
            for (ground, paper) in GROUNDS.iter().enumerate() {
                (studio.pages[0].look, studio.pages[0].ground) = (look as u8, ground as u8);
                studio.changed();
                let layers: Vec<Layer> = studio.doc.layers.clone();
                let sheet = studio.part_sheet(&layers, (10.0, 10.0, 740.0, 450.0));
                let (pw, ph, pixels) = sheet.render(times, &mut studio.text);
                let file = format!("{}-{}.png", name.to_lowercase(), paper.to_lowercase().replace(' ', "-"));
                std::fs::write(to.join(file), crate::output::encode(pw as u32, ph as u32, &pixels, false).unwrap()).unwrap();
            }
        }
    }
}

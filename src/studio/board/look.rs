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

/// A shape as a board of a look, on a ground, draws it: drawn `times` its
/// size, as an export does.
pub(in crate::studio) fn dress(shape: &mut Shape, look: u8, ground: u8, times: f32) {
    let boxy = matches!(shape.tool, Tool::Rect | Tool::Ellipse);
    match look {
        0 => {
            shape.rounded = shape.sketch && shape.tool == Tool::Rect;
            if shape.rounded {
                let (_, _, w, h) = shape.frame();
                shape.radius = (w.min(h) * 0.22).min(16.0 * times).max(2.0);
                shape.corners = None;
            }
        }
        2 => {
            // Clean: no hand, a hairline, rounded a little, in Geist.
            if shape.sketch {
                shape.sketch = false;
                shape.stroke = 1.25 * times;
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
        let style = page.board.then_some((page.look, page.ground));
        if self.doc.board != style {
            self.doc.board = style;
        }
        if self.active != LOOSE && self.pages[self.page].loose.board != style {
            self.pages[self.page].loose.board = style;
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
        let (look, ground) = (page.look, page.ground);
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
        // Rounded: its corner is paper, as an angular one's is not.
        let corner = shown(&mut studio, (103.0, 103.0));
        studio.set_look(Some(1), None);
        assert_ne!(shown(&mut studio, (103.0, 103.0)), corner);
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
}

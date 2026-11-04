//! How a board looks, for the page as a whole: by hand with rounded corners
//! and looser lines, as it begins; by hand and angular; or clean, hairline
//! borders, as Linear's and Vercel's are. And what it is on: light paper,
//! dark gray or black, every color of the board turning to its shade for
//! the dark, as Radix's light and dark scales do, so nothing is picked
//! again. Switching draws it again; nothing moves. The font its words are
//! in is a choice of its own, which the look leaves as it was.

use super::*;

/// The looks, and the grounds, by what each is called.
pub(in crate::studio) const LOOKS: [&str; 4] = ["Rounded", "Sketchy", "Clean", "Diagram"];
/// The look drawn as a diagram tool draws, FigJam's way: by where it is
/// among `LOOKS`.
pub(in crate::studio) const DIAGRAM: u8 = 3;
/// The light ground under the Diagram look: flat gray, no dots.
pub(in crate::studio) const DIAGRAM_PAPER: u32 = 0xF5F5F5;

/// Which of some layers are sections in the Diagram look: boxes with
/// another box, an oval, a picture or words wholly inside them. By where
/// each is among the layers.
pub(in crate::studio) fn sections(layers: &[Layer]) -> Vec<bool> {
    let frames: Vec<Option<(f32, f32, f32, f32)>> = layers.iter().map(|layer| match &layer.kind {
        Kind::Shape(shape) if !layer.hidden && matches!(shape.tool, Tool::Rect | Tool::Ellipse | Tool::Text) => Some(shape.frame()),
        Kind::Picture(picture) if !layer.hidden => {
            let corners = picture.corners();
            let (left, top) = (corners.iter().map(|c| c.0).fold(f32::MAX, f32::min), corners.iter().map(|c| c.1).fold(f32::MAX, f32::min));
            let (right, bottom) = (corners.iter().map(|c| c.0).fold(f32::MIN, f32::max), corners.iter().map(|c| c.1).fold(f32::MIN, f32::max));
            Some((left, top, right - left, bottom - top))
        }
        _ => None,
    }).collect();
    layers.iter().enumerate().map(|(index, layer)| {
        let Kind::Shape(shape) = &layer.kind else { return false };
        if shape.tool != Tool::Rect || shape.form != shapes::FORM_BOX || layer.hidden {
            return false;
        }
        let (x, y, w, h) = shape.frame();
        frames.iter().enumerate().any(|(other, frame)| {
            other != index && frame.is_some_and(|(fx, fy, fw, fh)| fx >= x && fy >= y && fx + fw <= x + w && fy + fh <= y + h && fw * fh < w * h)
        })
    }).collect()
}

/// The board's colors as the Diagram look has them, FigJam's: each line
/// color and each tint the board picks by name, and Mermaid's, given the
/// one FigJam draws in its place; any other as it is.
pub(in crate::studio) fn figjam(color: u32) -> u32 {
    const AS: [(u32, u32); 24] = [
        // Lines.
        (0xE03131, 0xCA3A31),
        (0xF08C00, 0xD9622B),
        (0xE8590C, 0xD9622B),
        (0xF59F00, 0xCC7C2E),
        (0x2F9E44, 0x4CA154),
        (0x0C8599, 0x0E9F9F),
        (0x1971C2, 0x3662E3),
        (0x7048E8, 0x743EE4),
        (0xC2255C, 0xC7317A),
        (0x868E96, 0x757575),
        // Tints.
        (0xFFE3E3, 0xFCF2F2),
        (0xFFE8CC, 0xFEF7EE),
        (0xFFF3BF, 0xFCF3CC),
        (0xD3F9D8, 0xEFFDF6),
        (0xC5F6FA, 0xEDFBFB),
        (0xD0EBFF, 0xEBF2FE),
        (0xE5DBFF, 0xF5F3FE),
        (0xFFDEEB, 0xFDF0F6),
        (0xF1F3F5, 0xF5F5F5),
        (0xE9ECEF, 0xF0F0F0),
        // Mermaid's own.
        (0x74C0FC, 0x3662E3),
        (0xB197FC, 0x743EE4),
        (0x8CE99A, 0x4CA154),
        (0xFFA8A8, 0xCA3A31),
    ];
    AS.iter().find(|(from, _)| *from == color & 0xFF_FFFF).map_or(color, |(_, to)| *to)
}

/// A board's paper and its dots, by its look and its ground: the Diagram
/// look's light ground a flat gray with no dots, as FigJam's canvas is.
pub(in crate::studio) fn paper_of(look: u8, ground: u8) -> (u32, u32) {
    match (look, ground) {
        (DIAGRAM, 0) => (DIAGRAM_PAPER, DIAGRAM_PAPER),
        _ => PAPERS[usize::from(ground.min(2))],
    }
}
pub(in crate::studio) const GROUNDS: [&str; 3] = ["Light", "Dark gray", "Black"];

/// A small box drawn in a look on a ground, as its list shows it: its
/// width, height and pixels.
pub(in crate::studio) fn look_sample(look: u8, ground: u8, text: &mut Text) -> (i32, i32, Vec<u32>) {
    let mut sheet = Doc::new(46, 24);
    sheet.background = Fill::Color(paper_of(look, ground).0);
    let mut shape = Shape::new(Tool::Rect, INK, LINE, (6.0, 4.0));
    shape.points[1] = (40.0, 20.0);
    // Called the same each time, to wobble the same.
    (shape.id, shape.sketch, shape.border, shape.radius, shape.filled, shape.fill) = (7, true, true, 6.0, true, FILLS[2]);
    sheet.layers.push(Layer::shape(shape));
    sheet.board = Some((look, ground, true));
    sheet.render(1.0, text)
}

/// Each ground's paper and its dots.
pub(in crate::studio) const PAPERS: [(u32, u32); 3] = [(shapes::PAPER, 0xD8D6CF), (0x232323, 0x3C3C3C), (0x000000, 0x2A2A2A)];

/// A board's colors and their shades on the dark: its line colors, its
/// fills, a note's edge, white and the paper.
const DARK: [(u32, u32); 34] = [
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
    (0xF59F00, 0xFFD43B),
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

/// The face a board's words were in before the font was its own choice:
/// Geist where it was drawn Clean, else the lettering.
pub(in crate::studio) fn implied_font(look: u8) -> usize {
    if look == 2 { 0 } else { LETTERING }
}

/// The look, ground and font last chosen, kept in `board-look` among
/// hotshot's state, for a new board to begin as.
pub(in crate::studio) fn last_look() -> (u8, u8, usize) {
    if cfg!(test) {
        return (0, 0, LETTERING);
    }
    let kept = crate::output::state_file("board-look").and_then(|file| std::fs::read_to_string(file).ok()).unwrap_or_default();
    let mut words = kept.split_whitespace().filter_map(|word| word.parse::<usize>().ok());
    let (look, ground) = (words.next().unwrap_or(0).min(usize::from(DIAGRAM)) as u8, words.next().unwrap_or(0).min(2) as u8);
    (look, ground, words.next().map_or(implied_font(look), |font| font.min(FACES.len() - 1)))
}

pub(in crate::studio) fn keep_look(look: u8, ground: u8, font: usize) {
    if cfg!(test) {
        return;
    }
    let Some(file) = crate::output::state_file("board-look") else { return };
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(file, format!("{look} {ground} {font}\n"));
}

/// A page made a board for the first time: drawn as the last board was,
/// its words in the font last chosen.
pub(in crate::studio) fn first_look(page: &mut Page) {
    if !page.styled {
        (page.look, page.ground, page.font) = last_look();
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
            // Clean: no hand, a hairline, rounded a little. Its words stay
            // in their own font, which is chosen apart from the look.
            if shape.sketch {
                shape.sketch = false;
                shape.stroke = 1.5 * times;
                if boxy {
                    shape.radius = 8.0 * times;
                }
            }
        }
        // Diagram: as FigJam draws, a strong edge 2.5 pixels wide round a
        // pale tint of it, corners rounded a little, words near black in
        // the board's font; its lines gray with open heads; the board's
        // colors FigJam's own.
        DIAGRAM => {
            let by = shape.scale.max(1e-3);
            shape.sketch = false;
            shape.rounded = false;
            shape.stroke = (shape.stroke * 1.25).max(1.0);
            shape.color = figjam(shape.color);
            shape.fill = figjam(shape.fill);
            if boxy {
                shape.radius = 6.0 * by * (shape.stroke / (2.5 * by)).clamp(0.5, 2.0);
                shape.corners = None;
                shape.words = 0x1E1E1E;
            }
            // A box holding others: a section, white with a faint edge, square.
            if shape.section && shape.tool == Tool::Rect {
                (shape.filled, shape.fill, shape.border, shape.hatch) = (true, 0xFFFFFF, true, false);
                (shape.color, shape.stroke, shape.radius, shape.dashed, shape.dotted) = (0xE4E4E4, by, 0.0, false, false);
                shape.words = 0x1A1A1A;
            }
            if matches!(shape.tool, Tool::Arrow | Tool::Line) {
                if shape.color == INK {
                    shape.color = 0x757575;
                }
                for end in 0..2 {
                    if shape.ends[end] == shapes::FILLED_HEAD {
                        shape.ends[end] = shapes::OPEN_HEAD;
                    }
                }
                shape.words = 0x1E1E1E;
            }
        }
        _ => {}
    }
    if ground > 0 {
        shape.color = shade(shape.color, ground);
        shape.fill = shade(shape.fill, ground);
        // Black kept a hair off nought, which says the light paper.
        shape.paper = paper_of(look, ground).0.max(1);
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
        let area = page.board.then(|| paper_of(page.look, page.ground).0);
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
        let was = page.look;
        if let Some(look) = look {
            page.look = look.min(DIAGRAM);
        }
        if let Some(ground) = ground {
            page.ground = ground.min(2);
        }
        page.styled = true;
        // A diagram is not hand lettered: words in the lettering are set in
        // Geist, the board's next words too; another font chosen is kept.
        if page.look == DIAGRAM && page.font == board::LETTERING {
            page.font = 0;
            for layer in &mut self.doc.layers {
                if let Kind::Shape(shape) = &mut layer.kind {
                    if shape.face == board::LETTERING {
                        shape.face = 0;
                        shape.touch();
                    }
                }
            }
        }
        // Its connectors go round corners, as a diagram's do: those bowed
        // between two things, when the look is chosen.
        let mut turned = false;
        if page.look == DIAGRAM && was != DIAGRAM {
            for layer in &mut self.doc.layers {
                if let Kind::Shape(shape) = &mut layer.kind {
                    if matches!(shape.tool, Tool::Arrow | Tool::Line) && shape.routed && shape.stuck.iter().all(Option::is_some) {
                        (shape.routed, shape.elbow) = (false, true);
                        shape.points.truncate(2);
                        shape.touch();
                        turned = true;
                    }
                }
            }
        }
        if turned {
            self.follow();
        }
        let page = &self.pages[self.page];
        let (look, ground, font) = (page.look, page.ground, page.font);
        keep_look(look, ground, font);
        self.shown.clear();
        self.shown_page.clear();
        self.changed();
        self.say(format!("Theme: {}, on {}", LOOKS[usize::from(look)], GROUNDS[usize::from(ground)].to_lowercase()));
    }

    /// The font a board's words are in: what is picked out set in it, and
    /// what is drawn next, on this board and on a new one; the look left
    /// as it was. One step to undo.
    pub(in crate::studio) fn set_font(&mut self, font: usize) {
        let font = font.min(FACES.len() - 1);
        self.checkpoint();
        let page = &mut self.pages[self.page];
        (page.font, page.styled) = (font, true);
        // Kept for a new board once chosen, not while only hovered.
        if self.preview.is_none() {
            keep_look(page.look, page.ground, font);
        }
        let styled: Vec<usize> = self.chosen().into_iter().filter(|&index| matches!(self.doc.layers[index].kind, Kind::Shape(_))).collect();
        for index in styled {
            if let Kind::Shape(shape) = &mut self.doc.layers[index].kind {
                shape.face = font;
                shape.touch();
            }
        }
        self.changed();
        self.say(format!("Font: {}", FACES[font]));
    }

    /// The font the board's next words are in.
    pub(in crate::studio) fn board_font(&self) -> usize {
        self.pages[self.page].font.min(FACES.len() - 1)
    }

    /// The paper of the board shown, and its dots.
    pub(in crate::studio) fn paper(&self) -> (u32, u32) {
        paper_of(self.pages[self.page].look, self.pages[self.page].ground)
    }
}

#[cfg(test)]
pub(in crate::studio) mod tests {
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
    fn a_diagram_board_is_on_plain_gray_in_geist_its_box_round_others_a_section_named_above_it() {
        let mut studio = board();
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (100.0, 120.0), (600.0, 420.0), Mods::default());
        type_in(&mut studio, "Gateway");
        press_key(&mut studio, Key::Escape, Mods::default());
        studio.selected.clear();
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (380.0, 300.0), (540.0, 380.0), Mods::default());
        type_in(&mut studio, "Auth");
        press_key(&mut studio, Key::Escape, Mods::default());
        studio.selected.clear();
        assert_eq!(LOOKS[DIAGRAM as usize], "Diagram");
        studio.set_look(Some(DIAGRAM), None);
        // Its font is Geist, for what is there and what comes next.
        assert_eq!(studio.board_font(), 0);
        let faces: Vec<usize> = studio.doc.layers.iter().filter_map(|layer| match &layer.kind {
            Kind::Shape(shape) => Some(shape.face),
            _ => None,
        }).collect();
        assert!(faces.iter().all(|&face| face == 0), "{faces:?}");
        let mut out = vec![0u32; (studio.width * studio.height) as usize];
        studio.draw(&mut out);
        let at = |studio: &Studio, (x, y): (f32, f32)| {
            let (x, y) = studio.to_window_page((x, y));
            out[(y * studio.width + x) as usize] & 0xFF_FFFF
        };
        // Plain gray paper: not a dot on it.
        let paper: Vec<u32> = (700..900).step_by(3).flat_map(|x| (100..400).step_by(3).map(move |y| (x as f32, y as f32))).map(|p| at(&studio, p)).filter(|&c| c != DIAGRAM_PAPER).collect();
        assert!(paper.is_empty(), "{:06X?}", &paper[..paper.len().min(5)]);
        // The box round another is a white section, its name in a gray tag
        // above its top left corner.
        assert_eq!(at(&studio, (150.0, 380.0)), 0xFFFFFF, "a section is white");
        let tag = (100..260).flat_map(|x| (80..118).map(move |y| (x as f32, y as f32))).filter(|&p| at(&studio, p) == 0xE6E6E6).count();
        assert!(tag > 200, "its tag above it: {tag}");
        // The box in it is not a section: no tag above it.
        let inner = (380..460).flat_map(|x| (270..298).map(move |y| (x as f32, y as f32))).filter(|&p| at(&studio, p) == 0xE6E6E6).count();
        assert_eq!(inner, 0);
        // And its tag is in what it covers.
        let section = shape(&studio, 0).clone();
        let (frame, bounds) = (section.frame(), section.local_bounds(&mut studio.text));
        assert!(bounds.y < frame.1 as i32 - 10, "its tag in its bounds: {bounds:?} {frame:?}");
    }

    #[test]
    fn a_diagram_s_connectors_go_round_corners_those_there_and_those_drawn_next() {
        let mut studio = board();
        for (from, to) in [((100.0, 100.0), (260.0, 172.0)), ((500.0, 300.0), (660.0, 372.0)), ((100.0, 500.0), (260.0, 572.0))] {
            press_key(&mut studio, Key::Char('r'), Mods::default());
            pull(&mut studio, from, to, Mods::default());
            press_key(&mut studio, Key::Escape, Mods::default());
            studio.selected.clear();
        }
        let bowed = studio.join(0, 1);
        assert!(shape(&studio, bowed).routed && !shape(&studio, bowed).elbow, "bowed in Rounded");
        studio.set_look(Some(DIAGRAM), None);
        assert!(shape(&studio, bowed).elbow && !shape(&studio, bowed).routed, "round corners once a diagram");
        assert!(shape(&studio, bowed).points.len() > 2, "a turn between them: {:?}", shape(&studio, bowed).points);
        let next = studio.join(1, 2);
        assert!(shape(&studio, next).elbow && !shape(&studio, next).routed, "and the next");
        studio.undo();
        assert!(shape(&studio, bowed).routed && !shape(&studio, bowed).elbow, "one step to undo");
    }

    #[test]
    fn a_connector_round_corners_between_boxes_well_apart_across_goes_side_to_side() {
        let mut studio = board();
        studio.set_look(Some(DIAGRAM), None);
        for (from, to) in [((40.0, 200.0), (240.0, 260.0)), ((420.0, 290.0), (640.0, 360.0))] {
            press_key(&mut studio, Key::Char('r'), Mods::default());
            pull(&mut studio, from, to, Mods::default());
            press_key(&mut studio, Key::Escape, Mods::default());
            studio.selected.clear();
        }
        let arrow = studio.join(0, 1);
        let points = shape(&studio, arrow).points.clone();
        // Out of the right of one and into the left of the other, a step
        // between, rather than round a corner into its top.
        assert!((points[0].0 - 240.0).abs() < 2.0 && (points[1].0 - 420.0).abs() < 2.0, "{points:?}");
    }

    #[test]
    fn diagram_colors_are_figjam_s() {
        assert_eq!(figjam(0x1971C2), 0x3662E3);
        assert_eq!(figjam(0xD0EBFF), 0xEBF2FE);
        assert_eq!(figjam(0xE03131), 0xCA3A31);
        assert_eq!(figjam(0x123456), 0x123456, "anything else as it is");
        assert_eq!(paper_of(DIAGRAM, 0), (DIAGRAM_PAPER, DIAGRAM_PAPER));
    }

    #[test]
    fn the_font_is_its_own_choice_apart_from_the_look_kept_in_the_file_and_used_for_what_is_drawn_next() {
        let mut studio = board();
        assert_eq!(studio.board_font(), LETTERING, "Architects Daughter to begin with");
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (100.0, 100.0), (300.0, 200.0), Mods::default());
        type_in(&mut studio, "Cache");
        press_key(&mut studio, Key::Escape, Mods::default());
        // Clean draws it crisp, its words still lettered.
        studio.selected.clear();
        studio.set_look(Some(2), None);
        assert_eq!((shape(&studio, 0).face, studio.board_font()), (LETTERING, LETTERING), "the look leaves the font");
        let lettered = shown_words(&mut studio);
        studio.set_look(Some(0), None);
        // Fira Code for the box picked out, and for the next; the look kept.
        studio.selected = vec![0];
        studio.set_font(3);
        assert_eq!((shape(&studio, 0).face, studio.board_font(), studio.pages[0].look), (3, 3, 0), "the font leaves the look");
        studio.set_look(Some(2), None);
        assert_ne!(shown_words(&mut studio), lettered, "drawn in Fira Code now");
        assert_eq!(shape(&studio, 0).face, 3);
        press_key(&mut studio, Key::Escape, Mods::default());
        studio.selected.clear();
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (400.0, 100.0), (560.0, 172.0), Mods::default());
        press_key(&mut studio, Key::Escape, Mods::default());
        assert_eq!(shape(&studio, 1).face, 3, "the next box in it");
        // With nothing picked out, only what is drawn next.
        studio.selected.clear();
        studio.set_font(0);
        assert_eq!((shape(&studio, 0).face, shape(&studio, 1).face, studio.board_font()), (3, 3, 0));
        // Kept in the file; one step to undo.
        let back = read_pages(&write_pages(&studio.gathered())).unwrap();
        assert_eq!((back[0].look, back[0].font), (2, 0));
        studio.undo();
        assert_eq!(studio.board_font(), 3);
        // A diagram brought in is in the board's font.
        assert!(studio.bring_mermaid("flowchart LR\n  API --> Database"));
        let boxes: Vec<Shape> = studio.doc.layers.iter().filter_map(|layer| match &layer.kind {
            Kind::Shape(shape) if shape.text == "API" || shape.text == "Database" => Some(shape.clone()),
            _ => None,
        }).collect();
        assert!(boxes.len() == 2 && boxes.iter().all(|shape| shape.face == 3), "Mermaid in Fira Code");
        crate::studio::board::mermaid::check::assert_words_fit(&mut studio);
    }

    #[test]
    fn the_font_is_chosen_from_its_own_list_in_the_bar_beside_the_look() {
        let mut studio = board();
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (300.0, 300.0), (460.0, 372.0), Mods::default());
        type_in(&mut studio, "Queue");
        press_key(&mut studio, Key::Escape, Mods::default());
        open_theme(&mut studio);
        let (look, font) = (studio.control_rect(Control::Board(Ctl::Look)).unwrap(), studio.control_rect(Control::Board(Ctl::Font)).unwrap());
        assert!(!look.intersects(font), "two controls");
        click(&mut studio, font.center());
        assert_eq!(studio.menu.map(|menu| menu.kind), Some(MenuKind::Fonts));
        let row = studio.menu_items(MenuKind::Fonts).iter().position(|item| matches!(item, menu::Item::Entry { says, .. } if says == "Instrument Serif")).unwrap();
        let rect = studio.control_rect(Control::MenuRow(0, row as u16)).unwrap();
        click(&mut studio, rect.center());
        assert_eq!((shape(&studio, 0).face, studio.board_font(), studio.pages[0].look), (1, 1, 0));
        // The look's list still only changes the look.
        choose(&mut studio, Ctl::Look, "Sketchy");
        assert_eq!((shape(&studio, 0).face, studio.board_font(), studio.pages[0].look), (1, 1, 1));
    }

    #[test]
    fn the_theme_paper_and_guessing_are_the_board_s_behind_a_button_at_its_bottom_right_not_in_the_panel_of_styles() {
        let mut studio = board();
        // Only its button, to begin with, with nothing picked out as with.
        let layout = studio.layout();
        let shut = layout.board_theme.expect("the board's settings");
        assert!(layout.board_bar.is_none());
        assert!(shut.right() > studio.width - 40 && shut.y > layout.dock.y - 10, "at the bottom right: {shut:?}");
        assert!(shut.w < 48, "only a button: {shut:?}");
        for ctl in [Ctl::Look, Ctl::Ground, Ctl::Guess] {
            assert!(studio.control_rect(Control::Board(ctl)).is_none(), "{ctl:?} not shown");
        }
        // A click opens them beside it.
        open_theme(&mut studio);
        let open = studio.layout().board_theme.unwrap();
        assert!(open.w > shut.w + 100 && open.right() == shut.right());
        for ctl in [Ctl::Look, Ctl::Ground, Ctl::Guess] {
            let place = studio.control_rect(Control::Board(ctl)).unwrap();
            assert!(open.contains(place.center().0, place.center().1), "{ctl:?} in it");
        }
        // A click on the board shuts them, and does what it does.
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (300.0, 300.0), (460.0, 372.0), Mods::default());
        press_key(&mut studio, Key::Escape, Mods::default());
        assert!(!studio.board.theme_open && studio.doc.layers.len() == 1, "shut, and the box drawn");
        // With a box picked out: its panel has none of them.
        open_theme(&mut studio);
        let layout = studio.layout();
        let panel = layout.board_bar.expect("its panel");
        for ctl in [Ctl::Look, Ctl::Ground, Ctl::Guess] {
            let place = studio.control_rect(Control::Board(ctl)).unwrap();
            assert!(!panel.intersects(place), "{ctl:?} not in the panel");
        }
        let font = studio.control_rect(Control::Board(Ctl::Font)).unwrap();
        assert!(panel.contains(font.center().0, font.center().1), "the font is the shape's");
        // Open while a list of theirs is chosen from.
        choose(&mut studio, Ctl::Look, "Diagram");
        assert_eq!(studio.pages[0].look, DIAGRAM);
        assert!(studio.board.theme_open);
        choose(&mut studio, Ctl::Ground, "Dark gray");
        assert_eq!(studio.pages[0].ground, 1);
    }

    #[test]
    fn what_the_theme_button_says_hovered_is_all_inside_the_window() {
        let mut studio = board();
        let button = studio.control_rect(Control::Board(Ctl::Theme)).unwrap();
        studio.pointer_moved(button.center().0, button.center().1, Mods::default());
        let mut out = vec![0u32; (studio.width * studio.height) as usize];
        studio.draw(&mut out);
        let dock = studio.layout().dock;
        // Its last column, from over the bar to the top of the button:
        // the paper, not the dark of the tip cut off by the edge.
        let x = studio.width - 2;
        let dark = (dock.y - 60..button.y).filter(|&y| out[(y * studio.width + x) as usize] & 0xFF < 0x80).count();
        assert_eq!(dark, 0);
        // And it is drawn, ending a little before the edge.
        let row = (dock.y - 60..button.y).find(|&y| (studio.width - 60..studio.width - 4).any(|x| out[(y * studio.width + x) as usize] & 0xFF < 0x40));
        assert!(row.is_some(), "its tip shown");
    }

    /// The board's theme, paper and guessing opened from their button.
    pub(in crate::studio) fn open_theme(studio: &mut Studio) {
        if !studio.board.theme_open {
            let place = studio.control_rect(Control::Board(Ctl::Theme)).unwrap();
            click(studio, place.center());
        }
        assert!(studio.board.theme_open);
    }

    /// A control of the bar clicked, and a row of the list it opens; the
    /// board's own opened first, for one of theirs.
    pub(in crate::studio) fn choose(studio: &mut Studio, ctl: Ctl, says: &str) {
        if matches!(ctl, Ctl::Look | Ctl::Ground | Ctl::Guess) {
            open_theme(studio);
        }
        let place = studio.control_rect(Control::Board(ctl)).unwrap();
        click(studio, place.center());
        let kind = studio.menu.map(|menu| menu.kind).expect("a list open");
        let row = studio.menu_items(kind).iter().position(|item| matches!(item, menu::Item::Entry { says: row, .. } if row == says)).unwrap_or_else(|| panic!("{says} in the list"));
        let rect = studio.control_rect(Control::MenuRow(0, row as u16)).unwrap();
        studio.pointer_moved(rect.center().0, rect.center().1, Mods::default());
        click(studio, rect.center());
    }

    #[test]
    fn the_look_and_the_ground_are_lists_that_open_from_the_bar_rather_than_switches() {
        let mut studio = board();
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (300.0, 300.0), (460.0, 372.0), Mods::default());
        press_key(&mut studio, Key::Escape, Mods::default());
        open_theme(&mut studio);
        for (ctl, kind, rows) in [(Ctl::Look, MenuKind::Looks, &LOOKS[..]), (Ctl::Ground, MenuKind::Grounds, &GROUNDS[..])] {
            let place = studio.control_rect(Control::Board(ctl)).unwrap();
            let (look, ground) = (studio.pages[0].look, studio.pages[0].ground);
            click(&mut studio, place.center());
            assert_eq!(studio.menu.map(|menu| menu.kind), Some(kind), "a click opens its list");
            assert_eq!((studio.pages[0].look, studio.pages[0].ground), (look, ground), "and changes nothing");
            let items = studio.menu_items(kind);
            let says: Vec<(String, bool)> = items.iter().filter_map(|item| match item {
                menu::Item::Entry { says, on, .. } => Some((says.clone(), *on)),
                _ => None,
            }).collect();
            assert_eq!(says.iter().map(|(says, _)| says.as_str()).collect::<Vec<_>>(), rows.to_vec());
            assert_eq!(says.iter().filter(|(_, on)| *on).count(), 1, "the one it is checked");
            // Hovering a row shows it on the board, and moving off puts it
            // back; Esc closes the list.
            let row = items.iter().position(|item| matches!(item, menu::Item::Entry { says, .. } if says == rows[2])).unwrap();
            let rect = studio.control_rect(Control::MenuRow(0, row as u16)).unwrap();
            studio.pointer_moved(rect.center().0, rect.center().1, Mods::default());
            let shown = if ctl == Ctl::Look { studio.pages[0].look } else { studio.pages[0].ground };
            assert_eq!(shown, 2, "previewed");
            studio.pointer_moved(rect.x - 300, rect.y - 300, Mods::default());
            assert_eq!((studio.pages[0].look, studio.pages[0].ground), (look, ground), "put back");
            press_key(&mut studio, Key::Escape, Mods::default());
            assert_eq!(studio.menu, None);
        }
        choose(&mut studio, Ctl::Ground, "Black");
        assert_eq!(studio.pages[0].ground, 2);
        choose(&mut studio, Ctl::Look, "Clean");
        assert_eq!((studio.pages[0].look, studio.pages[0].ground), (2, 2));
        studio.undo();
        assert_eq!((studio.pages[0].look, studio.pages[0].ground), (0, 2), "each one step");
    }

    #[test]
    fn a_file_from_before_the_font_was_its_own_looks_as_it_did() {
        let mut box_ = Shape::new(Tool::Rect, INK, LINE, (100.0, 100.0));
        box_.points[1] = (260.0, 172.0);
        (box_.text, box_.face, box_.sketch) = ("Cache".into(), LETTERING, true);
        let mut serif = box_.clone();
        serif.face = 1;
        let mut pages = vec![Page::new("Clean", Vec::new()), Page::new("Rounded", Vec::new())];
        for (page, look) in pages.iter_mut().zip([2, 0]) {
            (page.board, page.styled, page.look) = (true, true, look);
            page.loose.layers = vec![Layer::shape(box_.clone()), Layer::shape(serif.clone())];
        }
        // As it was written then: no font on its look's line.
        let then = String::from_utf8(write_pages(&pages)).unwrap().replace("board-look 2 0 2\n", "board-look 2 0\n").replace("board-look 0 0 2\n", "board-look 0 0\n");
        let back = read_pages(then.as_bytes()).unwrap();
        let faces = |page: &Page| page.loose.layers.iter().map(|layer| match &layer.kind {
            Kind::Shape(shape) => shape.face,
            Kind::Picture(_) => usize::MAX,
        }).collect::<Vec<_>>();
        assert_eq!((back[0].font, faces(&back[0])), (0, vec![0, 1]), "Clean's words in Geist outright, as they were drawn");
        assert_eq!((back[1].font, faces(&back[1])), (LETTERING, vec![LETTERING, 1]), "the rest lettered");
    }

    /// The pixels of the first box's words, as the board shows them.
    fn shown_words(studio: &mut Studio) -> Vec<u32> {
        let (x, y, w, h) = studio.box_frame(0);
        let mut out = vec![0u32; (studio.width * studio.height) as usize];
        studio.draw(&mut out);
        let (a, b) = (studio.to_window_page((x + 10.0, y + 10.0)), studio.to_window_page((x + w - 10.0, y + h - 10.0)));
        (a.1..b.1).flat_map(|py| (a.0..b.0).map(move |px| (px, py))).map(|(px, py)| out[(py * studio.width + px) as usize]).collect()
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
            // Across its left side, a quarter of the way down, clear of
            // the dot at its middle: a line about two pixels
            // wide, its edges blended with the paper, never darker than
            // its ink as edges over nothing once were.
            let row: Vec<u32> = (90..112).map(|x| shown(&mut studio, (x as f32, 118.0))).collect();
            let dark = row.iter().filter(|&&pixel| (pixel & 0xFF) < 0x80).count();
            assert!((1..=4).contains(&dark), "{}: {dark} dark pixels across its line", LOOKS[usize::from(look)]);
            assert!(row.iter().all(|&pixel| (pixel & 0xFF) >= 0x1A), "{}: an edge darker than its ink", LOOKS[usize::from(look)]);
        }
    }

    #[test]
    fn a_hand_drawn_box_is_gone_round_once_in_a_stroke_that_breathes_and_overlaps_where_it_began() {
        let mut studio = board();
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (100.0, 100.0), (260.0, 172.0), Mods::default());
        press_key(&mut studio, Key::Escape, Mods::default());
        for rounded in [false, true] {
            let mut box_ = shape(&studio, 0);
            box_.rounded = rounded;
            let strokes = box_.sketch_strokes();
            assert_eq!(strokes.len(), 1, "one stroke, round once");
            let ink = &strokes[0];
            // Nowhere more than a few pixels off the box: no tick marks
            // past its corners.
            assert!(ink.points.iter().all(|&(x, y)| (96.0..=264.0).contains(&x) && (96.0..=176.0).contains(&y)));
            // Its width swells and thins, and it eases on and off the paper
            // where it begins and ends, which are near each other.
            let (thin, thick) = ink.halves.iter().fold((f32::MAX, 0.0f32), |(a, b), &h| (a.min(h), b.max(h)));
            let middle = ink.halves[ink.halves.len() / 2];
            assert!(thick > thin * 1.2, "{thin}..{thick}");
            assert!(ink.halves[0] < middle * 0.8 && ink.halves[ink.halves.len() - 1] < middle * 0.8, "eased at the join");
            let (first, last) = (ink.points[0], ink.points[ink.points.len() - 1]);
            // Near the end it comes back by where it began, and goes on
            // a little past it.
            let far = |p: (f32, f32)| (p.0 - first.0).hypot(p.1 - first.1);
            let back = (ink.points.len() * 4 / 5..ink.points.len()).find(|&at| far(ink.points[at]) < 4.0);
            assert!(back.is_some_and(|at| at < ink.points.len() - 2), "on a little past where it began");
            assert!((2.0..20.0).contains(&far(last)));
        }
    }



    #[test]
    fn a_board_drawn_large_has_room_round_it_in_its_preview() {
        let mut studio = board();
        // Two boxes far apart, as drawn zoomed far out: wider than tall.
        for left in [0.0, 6000.0] {
            let mut shape = Shape::new(Tool::Rect, INK, LINE * 10.0, (left, 0.0));
            studio.board_dress(&mut shape);
            (shape.stroke, shape.points[1]) = (LINE * 10.0, (left + 2000.0, 1500.0));
            studio.doc.layers.push(Layer::shape(shape));
        }
        studio.changed();
        let pages = studio.gathered();
        let (w, h, pixels) = board_preview(&pages[0], 320, 200, &mut studio.text).expect("a preview");
        let paper = PAPERS[0].0;
        // Some columns of paper at either side, not the boxes' edges.
        for x in (0..6).chain(w - 6..w) {
            let column = (0..h).filter(|&y| pixels[(y * w + x) as usize] & 0xFF_FFFF != paper).count();
            assert_eq!(column, 0, "column {x} of {w} is paper");
        }
    }
    #[test]
    fn a_tall_board_s_preview_is_as_wide_as_asked_with_it_in_the_middle() {
        let mut studio = board();
        // Two boxes one well under the other: taller than wide.
        for top in [100.0, 700.0] {
            press_key(&mut studio, Key::Char('r'), Mods::default());
            pull(&mut studio, (100.0, top), (260.0, top + 72.0), Mods::default());
            press_key(&mut studio, Key::Escape, Mods::default());
            press_key(&mut studio, Key::Escape, Mods::default());
        }
        let pages = studio.gathered();
        let (w, h, pixels) = board_preview(&pages[0], 320, 200, &mut studio.text).expect("a preview");
        assert!(((w as f32 / h as f32) - 1.6).abs() < 0.05, "{w}x{h}");
        // The boxes in the middle across, paper either side.
        let paper = PAPERS[0].0;
        let column = |x: i32| (0..h).filter(|&y| pixels[(y * w + x) as usize] & 0xFF_FFFF != paper).count();
        assert_eq!((column(2), column(w - 3)), (0, 0), "paper at the sides");
        assert!(column(w / 2) > 0, "the boxes in the middle");
    }

    #[test]
    fn words_past_a_box_s_edge_are_drawn_on_the_board_as_in_its_preview() {
        let mut studio = board();
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (300.0, 200.0), (460.0, 272.0), Mods::default());
        press_key(&mut studio, Key::Escape, Mods::default());
        press_key(&mut studio, Key::Escape, Mods::default());
        let mut out = vec![0u32; (studio.width * studio.height) as usize];
        studio.selected.clear();
        studio.changed();
        studio.draw(&mut out);
        // Words set by hand larger than the box holds: past its foot.
        if let Kind::Shape(shape) = &mut studio.doc.layers[0].kind {
            (shape.text, shape.nominal, shape.font_px) = ("one\ntwo\nthree\nfour\nfive\nsix".into(), 0, 20);
            shape.touch();
        }
        // Drawn again only where it changed, as a step of typing is.
        studio.changed_layers(&[0]);
        studio.draw(&mut out);
        let shape = shape(&studio, 0);
        let (lines, high) = shape.label_lines(&mut studio.text);
        let (_, left, top, wide) = lines.last().expect("words").clone();
        assert!(top > 272.0, "the last line past the foot: {top}");
        let paper = studio.paper().0;
        let (a, b) = (studio.to_window_page((left, top)), studio.to_window_page((left + wide as f32, top + high as f32)));
        let inked = (a.1..b.1).flat_map(|y| (a.0..b.0).map(move |x| (x, y))).filter(|&(x, y)| out[(y * studio.width + x) as usize] & 0xFF_FFFF != paper).count();
        assert!(inked > 10, "the last line drawn: {inked} pixels of it");
    }
    #[test]
    fn a_dotted_line_drawn_by_hand_is_a_row_of_dots_all_along_it() {
        let mut studio = board();
        // A dotted edge of a Mermaid flowchart, as "A -.-> B" brings.
        assert!(studio.bring_mermaid("flowchart LR\n  A -.-> B"));
        let line = (0..studio.doc.layers.len()).filter_map(|at| match &studio.doc.layers[at].kind {
            Kind::Shape(shape) if shape.dotted || shape.dashed => Some(shape.clone()),
            _ => None,
        }).next().expect("a dotted edge");
        assert!(line.sketch);
        let along = (line.points[line.points.len() - 1].0 - line.points[0].0).hypot(line.points[line.points.len() - 1].1 - line.points[0].1);
        let dots = line.sketch_strokes().into_iter().filter(|ink| ink.points.len() == 2 && ink.points[0] == ink.points[1]).count();
        let every = line.stroke * 2.0 + 4.0 * line.scale;
        if line.dotted {
            assert!(dots as f32 >= along / every * 0.7, "{dots} dots over {along} px");
        }
        // However it is drawn, there is ink all along it, not only its head.
        let inked: f32 = line.sketch_strokes().iter().map(|ink| ink.points.len() as f32).sum();
        assert!(inked >= along / every * 0.7, "{inked} points of ink over {along} px");
    }

    /// A board of every kind of shape, with a Mermaid flowchart, written
    /// as a PNG into `$HOTSHOT_SNAPSHOTS` in each look on each ground, at
    /// `$HOTSHOT_TIMES` its size, or its own.
    ///
    ///     HOTSHOT_SNAPSHOTS=shots cargo test look_snapshots -- --ignored
    /// A diagram as a blog's figure would have it, in the Diagram look:
    ///
    ///     HOTSHOT_SNAPSHOTS=shots cargo test diagram_snapshot -- --ignored
    #[test]
    #[ignore]
    fn diagram_snapshot() {
        let Some(to) = std::env::var_os("HOTSHOT_SNAPSHOTS") else { return };
        let to = std::path::PathBuf::from(to);
        std::fs::create_dir_all(&to).unwrap();
        let mut studio = board();
        studio.set_look(Some(DIAGRAM), None);
        let mut add = |studio: &mut Studio, from: (f32, f32), to: (f32, f32), color: u32, fill: u32, words: &str| {
            let mut shape = Shape::new(Tool::Rect, INK, LINE, from);
            shape.points[1] = to;
            studio.board_dress(&mut shape);
            (shape.color, shape.fill, shape.filled) = (color, fill, true);
            shape.text = words.into();
            shape.touch();
            studio.doc.layers.push(Layer::shape(shape));
            studio.doc.layers.len() - 1
        };
        let gateway = add(&mut studio, (360.0, 60.0), (900.0, 520.0), INK, 0xFFFFFF, "MCP Gateway");
        let _ = gateway;
        let agents: Vec<usize> = ["Agent Builder", "Coding Agents", "Chat", "Workflows"].iter().enumerate()
            .map(|(i, words)| add(&mut studio, (40.0, 90.0 + i as f32 * 110.0), (240.0, 150.0 + i as f32 * 110.0), 0x7048E8, 0xE5DBFF, words)).collect();
        let registry = add(&mut studio, (420.0, 120.0), (640.0, 190.0), 0x1971C2, 0xD0EBFF, "Registry");
        let proxy = add(&mut studio, (420.0, 290.0), (640.0, 360.0), 0x1971C2, 0xD0EBFF, "Proxy");
        let auth = add(&mut studio, (700.0, 290.0), (860.0, 360.0), 0x2F9E44, 0xD3F9D8, "Auth");
        let crawler = add(&mut studio, (420.0, 420.0), (640.0, 490.0), 0xF08C00, 0xFFE8CC, "AutoCrawler");
        let services = add(&mut studio, (980.0, 290.0), (1160.0, 360.0), 0x868E96, 0xF1F3F5, "Services");
        for &agent in &agents {
            studio.join(agent, proxy);
        }
        for (from, to) in [(proxy, registry), (proxy, auth), (crawler, registry), (proxy, services)] {
            studio.join(from, to);
        }
        studio.selected.clear();
        studio.changed();
        let layers: Vec<Layer> = studio.doc.layers.clone();
        let sheet = studio.part_sheet(&layers, (0.0, 0.0, 1200.0, 560.0));
        let (pw, ph, pixels) = sheet.render(2.0, &mut studio.text);
        std::fs::write(to.join("diagram.png"), crate::output::encode(pw as u32, ph as u32, &pixels, false).unwrap()).unwrap();
    }

    /// A board's window with a box picked out, its panel and its bars:
    ///
    ///     HOTSHOT_SNAPSHOTS=shots cargo test window_snapshot -- --ignored
    #[test]
    #[ignore]
    fn window_snapshot() {
        let Some(to) = std::env::var_os("HOTSHOT_SNAPSHOTS") else { return };
        let to = std::path::PathBuf::from(to);
        std::fs::create_dir_all(&to).unwrap();
        let mut studio = board();
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, (300.0, 300.0), (460.0, 372.0), Mods::default());
        type_in(&mut studio, "Proxy");
        press_key(&mut studio, Key::Escape, Mods::default());
        let mut out = vec![0u32; (studio.width * studio.height) as usize];
        studio.draw(&mut out);
        std::fs::write(to.join("window.png"), crate::output::encode(studio.width as u32, studio.height as u32, &out, false).unwrap()).unwrap();
        open_theme(&mut studio);
        studio.draw(&mut out);
        std::fs::write(to.join("window-theme.png"), crate::output::encode(studio.width as u32, studio.height as u32, &out, false).unwrap()).unwrap();
    }

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

/// A small picture of a board page for the projects: all that is on it,
/// its own layers and what is in its frames, fitted into `wide` by `high`
/// on its paper, drawn as it is drawn on the board.
pub fn board_preview(page: &super::super::Page, wide: i32, high: i32, text: &mut Text) -> Option<(i32, i32, Vec<u32>)> {
    let mut layers: Vec<Layer> = page.loose.layers.iter().filter(|layer| !layer.hidden).cloned().collect();
    for frame in &page.frames {
        layers.extend(frame.layers.iter().filter(|layer| !layer.hidden).map(|layer| {
            let mut layer = layer.clone();
            layer.translate(frame.x, frame.y);
            layer
        }));
    }
    let paper = paper_of(page.look, page.ground).0;
    let boxes: Vec<(f32, f32, f32, f32)> = layers.iter().map(|layer| layer.bounds(text)).collect();
    // An empty board: its paper, blank.
    let Some((x, y, w, h)) = super::super::actions::union(&boxes) else {
        return Some((wide, high, vec![0xFF00_0000 | paper; (wide * high).max(0) as usize]));
    };
    // Room round it as large to the eye however large it was drawn: a
    // share of it, not so many pixels, which a board drawn zoomed far out
    // has next to none of at the size of a card.
    let room = (w.max(h) * 0.06).max(24.0);
    let (mut x, mut y, mut w, mut h) = (x - room, y - room, w + 2.0 * room, h + 2.0 * room);
    // As wide to as high as the picture asked for, what is on it in the
    // middle of its paper: a tall board is not a tall picture in a row of
    // wide ones.
    let shape = wide.max(1) as f32 / high.max(1) as f32;
    if w / h < shape {
        let wider = h * shape;
        (x, w) = (x - (wider - w) / 2.0, wider);
    } else {
        let taller = w / shape;
        (y, h) = (y - (taller - h) / 2.0, taller);
    }
    let mut sheet = Doc::new(w.ceil().max(1.0) as i32, h.ceil().max(1.0) as i32);
    sheet.background = Fill::Color(paper);
    sheet.layers = layers
        .into_iter()
        .map(|mut layer| {
            layer.translate(-x, -y);
            layer
        })
        .collect();
    sheet.board = Some((page.look, page.ground, true));
    let times = (wide as f32 / w).min(high as f32 / h).min(2.0);
    Some(sheet.render(times, text))
}

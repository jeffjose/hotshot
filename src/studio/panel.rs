//! The panel of properties down the right of the canvas: what there is to
//! set of what is picked out, or of the frame, a section at a time, laid
//! out as Figma lays its out: a heading, then rows of numbers, colors,
//! lists and buttons, each row 26 pixels high.

use super::*;

/// How high a field is, and how far down the next row begins.
pub(super) const FIELD_H: i32 = 26;
const ROW_STEP: i32 = 32;
/// How much of a row the word at its left takes, saying what it is for.
const LABEL_W: i32 = 54;

/// The rows of the panel as they are laid out, down from `y`.
pub(super) struct Rows<'a> {
    pub layout: &'a mut Layout,
    pub left: i32,
    pub inner: i32,
    pub y: i32,
    pub rule_x: i32,
    pub panel_w: i32,
}

impl Rows<'_> {
    /// A heading, with a line over it, a word or two of what it is about
    /// after it, and a button at its right (a plus, mostly).
    pub fn section(&mut self, title: &str, hint: &str, button: Option<(Control, &'static str)>) {
        self.layout.rules.push(Rect::new(self.rule_x, self.y, self.panel_w, 1));
        self.y += 10;
        self.layout.words.push(((self.left, self.y + 4), title.to_owned(), TEXT_COLOR, 12));
        if !hint.is_empty() {
            self.layout.words.push(((self.left + 10 + hint_offset(title), self.y + 5), hint.to_owned(), DIM_TEXT_COLOR, 11));
        }
        if let Some((control, _)) = button {
            let place = Rect::new(self.left + self.inner - 22, self.y, 22, 22);
            self.layout.controls.push((place, control));
        }
        self.y += 28;
    }

    /// Numbers side by side, each with its label to drag it by; and after
    /// them, buttons of their own, square.
    pub fn fields(&mut self, row: &[Field], buttons: &[Control]) {
        let count = row.len() as i32;
        let room = self.inner - buttons.len() as i32 * (FIELD_H + 6);
        let each = (room - 8 * (count - 1)) / count.max(1);
        for (index, &field) in row.iter().enumerate() {
            self.layout.controls.push((Rect::new(self.left + index as i32 * (each + 8), self.y, each, FIELD_H), Control::Field(field)));
        }
        for (index, &button) in buttons.iter().enumerate() {
            let x = self.left + self.inner - (buttons.len() - index) as i32 * (FIELD_H + 6) + 6;
            self.layout.controls.push((Rect::new(x, self.y, FIELD_H, FIELD_H), button));
        }
        self.y += ROW_STEP;
    }

    /// Square buttons in groups, a gap between groups, as the alignment
    /// row has them.
    pub fn buttons(&mut self, groups: &[&[Control]]) {
        let mut x = self.left;
        for group in groups {
            for &button in group.iter() {
                self.layout.controls.push((Rect::new(x, self.y, FIELD_H, FIELD_H), button));
                x += FIELD_H + 2;
            }
            x += 8;
        }
        self.y += ROW_STEP;
    }

    /// Chips, as many to a row as fit, each as wide as what it says.
    pub fn chips(&mut self, text: &mut Text, items: Vec<(String, Control)>) {
        let px = 12;
        let line = Text::line_height(px);
        let mut x = self.left;
        for (says, control) in items {
            let wide = text.width(&says, px) + 18;
            if x + wide > self.left + self.inner && x > self.left {
                (x, self.y) = (self.left, self.y + 30);
            }
            self.layout.controls.push((Rect::new(x, self.y, wide, FIELD_H), control));
            self.layout.words.push(((x + 9, self.y + (FIELD_H - line) / 2), says, TEXT_COLOR, px));
            x += wide + 5;
        }
        self.y += ROW_STEP;
    }

    /// Square buttons in groups, then chips after them, on one row.
    pub fn tools(&mut self, text: &mut Text, groups: &[&[Control]], items: Vec<(String, Control)>) {
        let mut x = self.left;
        for group in groups {
            for &button in group.iter() {
                self.layout.controls.push((Rect::new(x, self.y, FIELD_H, FIELD_H), button));
                x += FIELD_H + 2;
            }
            x += 3;
        }
        let line = Text::line_height(12);
        for (says, control) in items {
            let wide = text.width(&says, 12) + 14;
            self.layout.controls.push((Rect::new(x, self.y, wide, FIELD_H), control));
            self.layout.words.push(((x + 7, self.y + (FIELD_H - line) / 2), says, TEXT_COLOR, 12));
            x += wide + 4;
        }
        self.y += ROW_STEP;
    }

    /// Chips of the same width, so many to a row, what each says in its
    /// middle.
    pub fn grid(&mut self, text: &mut Text, items: Vec<(String, Control)>, across: usize) {
        let across = across.max(1) as i32;
        let wide = (self.inner - 6 * (across - 1)) / across;
        let line = Text::line_height(12);
        for (at, (says, control)) in items.into_iter().enumerate() {
            let (column, row) = (at as i32 % across, at as i32 / across);
            if column == 0 && row > 0 {
                self.y += ROW_STEP;
            }
            let x = self.left + column * (wide + 6);
            self.layout.controls.push((Rect::new(x, self.y, wide, FIELD_H), control));
            let says_wide = text.width(&says, 12);
            self.layout.words.push(((x + (wide - says_wide) / 2, self.y + (FIELD_H - line) / 2), says, TEXT_COLOR, 12));
        }
        self.y += ROW_STEP;
    }

    /// What a row is for, at its left, and chips after it, each as wide
    /// as what it says, as tabs are.
    pub fn labeled(&mut self, text: &mut Text, label: &str, items: Vec<(String, Control)>) {
        let line = Text::line_height(12);
        self.layout.words.push(((self.left, self.y + (FIELD_H - Text::line_height(11)) / 2), label.to_owned(), DIM_TEXT_COLOR, 11));
        let mut x = self.left + LABEL_W;
        for (says, control) in items {
            let wide = text.width(&says, 12) + 16;
            self.layout.controls.push((Rect::new(x, self.y, wide, FIELD_H), control));
            self.layout.words.push(((x + 8, self.y + (FIELD_H - line) / 2), says, TEXT_COLOR, 12));
            x += wide + 4;
        }
        self.y += ROW_STEP;
    }

    /// What a row is for, at its left, and one control taking the rest.
    pub fn labeled_wide(&mut self, label: &str, control: Control) -> Rect {
        self.layout.words.push(((self.left, self.y + (FIELD_H - Text::line_height(11)) / 2), label.to_owned(), DIM_TEXT_COLOR, 11));
        let place = Rect::new(self.left + LABEL_W, self.y, self.inner - LABEL_W, FIELD_H);
        self.layout.controls.push((place, control));
        self.y += ROW_STEP;
        place
    }

    /// A small heading in capitals, over a row.
    pub fn label(&mut self, says: &str) {
        self.layout.words.push(((self.left, self.y), says.to_owned(), DIM_TEXT_COLOR, 10));
        self.y += Text::line_height(10) + 6;
    }

    /// A whole-width control: a color, a list, a name to type.
    pub fn wide(&mut self, control: Control) -> Rect {
        let place = Rect::new(self.left, self.y, self.inner, FIELD_H);
        self.layout.controls.push((place, control));
        self.y += ROW_STEP;
        place
    }

    /// Controls side by side, sharing the row by the widths given, a
    /// width of 0 taking what is left.
    pub fn split(&mut self, items: &[(Control, i32)]) -> Vec<Rect> {
        let fixed: i32 = items.iter().map(|(_, w)| *w).sum::<i32>() + 6 * (items.len() as i32 - 1);
        let flexible = items.iter().filter(|(_, w)| *w == 0).count().max(1) as i32;
        let share = ((self.inner - fixed) / flexible).max(30);
        let mut x = self.left;
        let mut places = Vec::new();
        for &(control, w) in items {
            let w = if w == 0 { share } else { w };
            let place = Rect::new(x, self.y, w, FIELD_H);
            self.layout.controls.push((place, control));
            places.push(place);
            x += w + 6;
        }
        self.y += ROW_STEP;
        places
    }

    /// A word or two, small and faint.
    pub fn note(&mut self, says: &str) {
        self.layout.words.push(((self.left, self.y), says.to_owned(), DIM_TEXT_COLOR, 11));
        self.y += Text::line_height(11) + 8;
    }
}

/// How far along a heading its hint goes: past its words.
fn hint_offset(title: &str) -> i32 {
    title.chars().count() as i32 * 7
}

impl Studio {
    /// The sections of the panel of properties, from `y` down: what is
    /// picked out, or the frame. Gives where they end.
    pub(super) fn properties(&mut self, layout: &mut Layout, left: i32, inner: i32, y: i32, panel_w: i32) -> i32 {
        let mut rows = Rows { layout, left, inner, y, rule_x: left - 14, panel_w };
        let act = Control::Action;
        let actions = |list: &[Action]| list.iter().map(|&action| (action.says().to_owned(), Control::Action(action))).collect::<Vec<_>>();
        if self.panel_tab == 1 {
            self.export_sections(&mut rows);
            return rows.y;
        }
        if self.cropping.is_some() {
            rows.section("Crop", "", None);
            rows.note("Drag the corners, or the picture inside");
            let aspects = (0..CROP_ASPECTS.len()).map(|index| (CROP_ASPECTS[index].0.to_owned(), act(Action::Aspect(index as u8)))).collect();
            rows.chips(&mut self.text, aspects);
            rows.chips(&mut self.text, vec![("Tight".into(), act(Action::Tight)), ("Reset".into(), act(Action::ResetCrop)), ("Done".into(), act(Action::Crop))]);
            rows.note("Tight again goes further in, Shift out");
            return rows.y;
        }
        let chosen = self.chosen();
        // The page's own layers worked in, none picked out: no frame to set.
        if chosen.is_empty() && self.on_page() {
            let page = self.pages[self.page].name.clone();
            rows.section("Page", &page, None);
            rows.note("Layers outside the frames are drawn on the");
            rows.note("page and go in no export; drag one into a");
            rows.note("frame to put it in that frame");
            return rows.y;
        }
        // Lining up one picked out, on the frame; several have more below.
        // One of the page's own layers has no frame to line up on.
        if chosen.len() == 1 && !self.on_page() {
            rows.y += 8;
            let aligns: Vec<Control> = (0..6).map(|way| act(Action::Align(way))).collect();
            rows.buttons(&[&aligns[..3], &aligns[3..], &[act(Action::Distribute(0)), act(Action::Distribute(1))]]);
        }
        match chosen[..] {
            [] => self.frame_sections(&mut rows),
            [index] => match self.doc.layers[index].kind.clone() {
                Kind::Picture(picture) => {
                    // How large it is shown, against its own pixels.
                    let times = picture.w / picture.crop.2.max(1.0);
                    rows.section("Picture", &format!("{} \u{d7} {}  \u{b7}  {}\u{d7} of its pixels", picture.image.width(), picture.image.height(), (times * 100.0).round() / 100.0), None);
                    rows.fields(&[Field::X, Field::Y, Field::Turn], &[]);
                    rows.fields(&[Field::W, Field::H], &[act(Action::LockRatio)]);
                    rows.buttons(&[&[act(Action::FlipAcross), act(Action::FlipDown)], &[act(Action::Crop)], &[act(Action::Actual)]]);
                    rows.section("Appearance", "", None);
                    self.radius_rows(&mut rows);
                    rows.section("Adjust", "", Some((act(Action::ResetAdjust), icons::UNDO)));
                    for field in [Field::Exposure, Field::Contrast, Field::Saturation] {
                        rows.wide(Control::Field(field));
                    }
                    self.window_section(&mut rows, index, &picture);
                    self.effects_section(&mut rows, picture.shadow != 0);
                    self.layer_section(&mut rows);
                }
                Kind::Shape(shape) => self.shape_sections(&mut rows, index, &shape),
            },
            _ => {
                // What they are, a word for each kind, the commonest first.
                let mut kinds: Vec<(String, usize)> = Vec::new();
                for &index in &chosen {
                    let kind = match &self.doc.layers[index].kind {
                        Kind::Picture(_) => "picture".to_owned(),
                        Kind::Shape(shape) => tool_name(shape.tool).to_lowercase(),
                    };
                    match kinds.iter_mut().find(|(known, _)| *known == kind) {
                        Some((_, count)) => *count += 1,
                        None => kinds.push((kind, 1)),
                    }
                }
                let kinds: Vec<String> = kinds.into_iter().map(|(kind, _)| kind).take(3).collect();
                rows.section(&format!("{} layers", chosen.len()), &kinds.join(", "), None);
                rows.fields(&[Field::X, Field::Y], &[]);
                rows.fields(&[Field::W, Field::H], &[act(Action::LockRatio)]);
                // Every color among them, with how many have it: changed at
                // Lining up and spacing: for a row and for a column, the
                // one they lie in first.
                let boxes: Vec<(f32, f32, f32, f32)> = chosen.iter().map(|&index| self.doc.layers[index].bounds(&mut self.text)).collect();
                let row = arrange::in_a_row(&boxes);
                let mut ways = [
                    ("FOR A ROW", [act(Action::Align(3)), act(Action::Align(4)), act(Action::Align(5))], actions(&[Action::Distribute(0), Action::Pack(0)])),
                    ("FOR A COLUMN", [act(Action::Align(0)), act(Action::Align(1)), act(Action::Align(2))], actions(&[Action::Distribute(1), Action::Pack(1)])),
                ];
                if !row {
                    ways.swap(0, 1);
                }
                rows.section("Arrange", if row { "in a row" } else { "in a column" }, None);
                for (says, buttons, chips) in ways {
                    rows.label(says);
                    rows.tools(&mut self.text, &[&buttons], chips);
                }
                rows.section("Match to 1", "the first picked", None);
                rows.grid(&mut self.text, actions(&(0..6).map(Action::Match).collect::<Vec<_>>()), 3);
                if !self.on_page() {
                    rows.section("To the frame", "", None);
                    let to_frame: Vec<Control> = (0..6).map(|way| act(Action::ToFrame(way))).collect();
                    rows.tools(&mut self.text, &[&to_frame[..3], &to_frame[3..]], actions(&[Action::ToFrame(6)]));
                }
                rows.section("Turn", "", None);
                rows.grid(&mut self.text, actions(&[Action::Turn(-90), Action::Turn(90), Action::Turn(180), Action::FlipAcross]), 4);
                // Every color among them, once, with how many layers use
                // it: changed at once for all of them, or opened to them.
                let colors = self.selection_colors();
                if !colors.is_empty() {
                    rows.section("Colors in the selection", "", None);
                    for &(color, _) in colors.iter().take(8) {
                        rows.split(&[(Control::ColorField(Paint::Recolor(color)), 0), (Control::ColorOpen(color), FIELD_H)]);
                        if self.color_open == Some(color) {
                            for (index, role) in self.color_uses(color) {
                                rows.wide(Control::ColorLayer(index, role));
                            }
                        }
                    }
                }
                rows.section("Together", "", None);
                rows.chips(&mut self.text, actions(&[Action::FrameThem]));
                self.layer_section(&mut rows);
            }
        }
        rows.y
    }

    /// The panel with nothing picked out: the frame.
    fn frame_sections(&mut self, rows: &mut Rows) {
        let name = self.doc.name.clone();
        rows.section("Frame", &name, None);
        rows.wide(Control::Menu(MenuKind::Sizes));
        rows.fields(&[Field::FrameX, Field::FrameY], &[]);
        rows.fields(&[Field::ArtW, Field::ArtH], &[Control::Action(Action::Sideways)]);
        rows.section("Background", "", None);
        // The kinds a shape's fill has too, as Figma names them.
        let grounds = [("Solid", Ground::Color), ("Linear", Ground::Gradient), ("Radial", Ground::Radial), ("Backdrop", Ground::Backdrop), ("None", Ground::None)];
        rows.chips(&mut self.text, grounds.iter().map(|&(says, ground)| (says.to_owned(), Control::Ground(ground))).collect());
        match self.doc.background {
            Fill::Color(_) => drop(rows.wide(Control::ColorField(Paint::Background))),
            Fill::Gradient(..) | Fill::Radial(..) => {
                rows.wide(Control::GradientBar);
                rows.wide(Control::ColorField(Paint::From));
                rows.wide(Control::ColorField(Paint::To));
                if matches!(self.doc.background, Fill::Gradient(..)) {
                    rows.fields(&[Field::Angle], &[]);
                }
            }
            Fill::Backdrop(_) => {
                let (each, across) = (rows.inner / 6, 6);
                for which in 0..BACKDROPS.len() {
                    rows.layout.controls.push((Rect::new(rows.left + which as i32 % across * each, rows.y + which as i32 / across * each, each - 4, each - 4), Control::Backdrop(which)));
                }
                rows.y += BACKDROPS.len().div_ceil(across as usize) as i32 * each + 6;
            }
            Fill::None => {}
        }
        self.export_sizes(rows);
        let says = format!("Export {}", self.doc.name);
        // Copy is a PNG, as other programs paste nothing else yet; Copy as
        // file hands over the file the first row writes.
        let act = |action: Action| (action.says().to_owned(), Control::Action(action));
        rows.chips(&mut self.text, vec![(says, Control::Action(Action::Export)), act(Action::Copy), act(Action::CopyFile), act(Action::FineTune(0))]);
    }

    /// The panel with one shape picked out.
    fn shape_sections(&mut self, rows: &mut Rows, index: usize, shape: &Shape) {
        let act = Control::Action;
        let actions = |list: &[Action]| list.iter().map(|&action| (action.says().to_owned(), Control::Action(action))).collect::<Vec<_>>();
        // A blur and the like have sections of their own.
        if effects::changes_beneath(shape.tool) {
            self.effect_sections(rows, shape);
            self.layer_section(rows);
            return;
        }
        let name = tool_name(shape.tool);
        rows.section(name, "", None);
        let line = matches!(shape.tool, Tool::Line | Tool::Arrow);
        if line {
            rows.fields(&[Field::StartX, Field::StartY], &[]);
            rows.fields(&[Field::EndX, Field::EndY], &[]);
            rows.fields(&[Field::Length, Field::LineAngle], &[]);
        } else {
            if shape.turnable() {
                rows.fields(&[Field::X, Field::Y, Field::Turn], &[]);
            } else {
                rows.fields(&[Field::X, Field::Y], &[]);
            }
            if matches!(shape.tool, Tool::Rect | Tool::Ellipse) {
                rows.fields(&[Field::W, Field::H], &[act(Action::LockRatio)]);
            }
            if shape.turns() {
                rows.buttons(&[&[act(Action::FlipAcross), act(Action::FlipDown)]]);
            }
        }
        rows.section("Appearance", "", None);
        if shape.tool == Tool::Rect {
            self.radius_rows(rows);
        } else {
            rows.fields(&[Field::Opacity], &[]);
        }
        match shape.tool {
            Tool::Rect | Tool::Ellipse => {
                let filled = shape.filled;
                rows.section("Fill", "", Some(if filled { (act(Action::FillOff), icons::MINUS) } else { (act(Action::FillOn), icons::PLUS) }));
                if filled {
                    // One color, or a gradient from one to another, as
                    // the frame's background is chosen.
                    rows.chips(&mut self.text, (0..3).map(|kind| (Action::FillKind(kind).says().to_owned(), act(Action::FillKind(kind)))).collect());
                    match shape.gradient {
                        None => drop(rows.wide(Control::ColorField(Paint::Fill))),
                        Some(gradient) => {
                            rows.wide(Control::FillBar);
                            rows.wide(Control::ColorField(Paint::Fill));
                            rows.wide(Control::ColorField(Paint::FillTo));
                            if !gradient.radial {
                                rows.fields(&[Field::FillAngle], &[]);
                            }
                        }
                    }
                } else {
                    rows.note("No fill: what is under it shows");
                }
                let border = shape.border || !filled;
                rows.section("Stroke", "", Some(if border { (act(Action::BorderOff), icons::MINUS) } else { (act(Action::BorderOn), icons::PLUS) }));
                if border {
                    rows.wide(Control::ColorField(Paint::Line));
                    rows.split(&[(Control::Menu(MenuKind::Edge), 0), (Control::Field(Field::Stroke), 76)]);
                    rows.split(&[(Control::Menu(MenuKind::Dash), 0), (Control::Menu(MenuKind::Hand), 0)]);
                }
            }
            Tool::Text | Tool::Badge => {
                rows.section("Typography", "", None);
                rows.split(&[(Control::Menu(MenuKind::Fonts), 0), (Control::Field(Field::FontSize), 76)]);
                rows.buttons(&[&[act(Action::TextAlign(0)), act(Action::TextAlign(1)), act(Action::TextAlign(2))], &[act(Action::Bold), act(Action::Italic), act(Action::Underline), act(Action::Strike)]]);
                rows.buttons(&[&[act(Action::Upper)], &[act(Action::Highlight), act(Action::Bullets)]]);
                rows.note("Lines starting # , ## or ### are headings");
                rows.section("Color", "", None);
                rows.wide(Control::ColorField(Paint::Line));
                rows.section("Background", "behind the text", None);
                rows.chips(&mut self.text, vec![("None".into(), act(Action::Pill(false))), ("Pill".into(), act(Action::Pill(true)))]);
            }
            _ => {
                rows.section("Stroke", "", None);
                rows.split(&[(Control::ColorField(Paint::Line), 0), (Control::Field(Field::Stroke), 76)]);
                if line {
                    rows.split(&[(Control::Menu(MenuKind::Dash), 0), (Control::Menu(MenuKind::Hand), 0)]);
                    rows.chips(&mut self.text, actions(&[Action::Halo]));
                }
            }
        }
        if line {
            rows.section("Ends", "", None);
            rows.split(&[(Control::Menu(MenuKind::Heads(0)), 0), (act(Action::Swap), FIELD_H), (Control::Menu(MenuKind::Heads(1)), 0)]);
            rows.section("Path", "", None);
            rows.chips(&mut self.text, vec![("Straight".into(), act(Action::Path(0))), ("Curved".into(), act(Action::Path(1))), ("Elbow".into(), act(Action::Path(2)))]);
            rows.section("Stuck to", "", None);
            for end in 0..2 {
                rows.split(&[(Control::Stuck(end as u8), 0), (act(Action::Unstick(end as u8)), FIELD_H)]);
            }
        }
        let _ = index;
        self.effects_section(rows, shape.shadow.is_some());
        self.layer_section(rows);
    }

    /// How opaque it is and how round its corners, with a button that
    /// shows a radius for each corner, as Figma's does: top left and right
    /// over bottom left and right.
    fn radius_rows(&mut self, rows: &mut Rows) {
        rows.fields(&[Field::Opacity, Field::Radius], &[Control::Action(Action::Corners)]);
        if self.corners_open {
            rows.fields(&[Field::Corner(0), Field::Corner(1)], &[]);
            rows.fields(&[Field::Corner(3), Field::Corner(2)], &[]);
        }
    }

    /// The shadow a layer casts: a plus for one, or which it is and its
    /// numbers, its color, and a minus.
    fn effects_section(&mut self, rows: &mut Rows, on: bool) {
        let act = Control::Action;
        rows.section("Effects", "", Some(if on { (act(Action::ShadowOff), icons::MINUS) } else { (act(Action::ShadowOn), icons::PLUS) }));
        if on {
            rows.wide(Control::Menu(MenuKind::Shadows));
            rows.fields(&[Field::DropY, Field::DropBlur, Field::DropSpread], &[]);
            rows.split(&[(Control::ColorField(Paint::Shadow), 0), (Control::Field(Field::DropAlpha), 76)]);
        }
    }

    /// What can be done with any layer: over and under the rest, a copy,
    /// and away.
    fn layer_section(&mut self, rows: &mut Rows) {
        rows.section("Layer", "", None);
        let items = [Action::Forward, Action::Backward, Action::Duplicate, Action::Delete].iter().map(|&action| (action.says().to_owned(), Control::Action(action))).collect();
        rows.chips(&mut self.text, items);
    }

    /// The colors of the shapes picked out, each once with how many
    /// layers use it, the commonest first.
    pub(super) fn selection_colors(&self) -> Vec<(u32, usize)> {
        let mut found: Vec<(u32, Vec<usize>)> = Vec::new();
        for index in self.chosen() {
            for (color, _) in self.colors_of(index) {
                match found.iter_mut().find(|(known, _)| *known == color) {
                    Some((_, layers)) if layers.contains(&index) => {}
                    Some((_, layers)) => layers.push(index),
                    None => found.push((color, vec![index])),
                }
            }
        }
        let mut found: Vec<(u32, usize)> = found.into_iter().map(|(color, layers)| (color, layers.len())).collect();
        found.sort_by(|a, b| b.1.cmp(&a.1));
        found
    }

    /// The colors a layer is in, each with what it colors: its line, its
    /// fill, its letters, its pill.
    pub(super) fn colors_of(&self, index: usize) -> Vec<(u32, &'static str)> {
        let Some(Layer { kind: Kind::Shape(shape), .. }) = self.doc.layers.get(index) else { return Vec::new() };
        match shape.tool {
            Tool::Rect | Tool::Ellipse => {
                let mut colors = Vec::new();
                if shape.border || !shape.filled {
                    colors.push((shape.color, "stroke"));
                }
                if shape.filled {
                    colors.push((shape.fill, "fill"));
                    if let Some(gradient) = shape.gradient {
                        colors.push((gradient.to, "fill"));
                    }
                }
                colors
            }
            Tool::Text if shape.style & shapes::BOXED != 0 => vec![(shape.color, "pill")],
            Tool::Text => vec![(shape.color, "text")],
            Tool::Counter | Tool::Badge => vec![(shape.color, "fill")],
            // A blur has no color of its own.
            Tool::Blur | Tool::Spotlight => Vec::new(),
            _ => vec![(shape.color, "stroke")],
        }
    }

    /// The layers picked out that use a color, each with what it colors.
    pub(super) fn color_uses(&self, color: u32) -> Vec<(usize, &'static str)> {
        self.chosen().into_iter().flat_map(|index| self.colors_of(index).into_iter().filter(move |(known, _)| *known == color).map(move |(_, role)| (index, role))).collect()
    }
}

/// The shapes a crop can be kept to, by what each is called: free, the
/// picture's own, and the usual ones.
pub(super) const CROP_ASPECTS: [(&str, f32); 6] = [("Free", 0.0), ("Original", -1.0), ("16:9", 16.0 / 9.0), ("4:3", 4.0 / 3.0), ("1:1", 1.0), ("9:16", 9.0 / 16.0)];

impl Studio {
    /// Whether a button or chip of the panel is lit: what it sets is what
    /// there is.
    pub(super) fn action_on(&self, action: Action, shown: &Option<Layer>) -> bool {
        let shape = match shown {
            Some(Layer { kind: Kind::Shape(shape), .. }) => Some(shape),
            _ => None,
        };
        match action {
            Action::LockRatio => self.keep_ratio,
            Action::Corners => self.corners_open,
            Action::FillKind(kind) => shape.and_then(Studio::fill_kind) == Some(kind),
            Action::Halo => shape.is_some_and(|shape| shape.halo),
            Action::Crop => self.cropping.is_some(),
            Action::Bold => shape.is_some_and(|shape| shape.style & shapes::BOLD != 0),
            Action::Italic => shape.is_some_and(|shape| shape.style & shapes::ITALIC != 0),
            Action::Highlight => shape.is_some_and(|shape| shape.style & shapes::HIGHLIGHT != 0),
            Action::Bullets => shape.is_some_and(|shape| shape.style & shapes::BULLETS != 0),
            Action::Underline => shape.is_some_and(|shape| shape.style & shapes::UNDERLINE != 0),
            Action::Strike => shape.is_some_and(|shape| shape.style & shapes::STRIKE != 0),
            Action::Upper => shape.is_some_and(|shape| shape.style & shapes::UPPER != 0),
            Action::Pill(on) => shape.is_some_and(|shape| (shape.style & shapes::BOXED != 0) == on),
            Action::TextAlign(align) => shape.is_some_and(|shape| shape.align == align),
            Action::Path(path) => shape.is_some_and(|shape| match path {
                2 => shape.elbow,
                1 => !shape.elbow && shape.points.len() > 2,
                _ => !shape.elbow && shape.points.len() <= 2,
            }),
            Action::Aspect(which) => usize::from(which) == self.crop_aspect,
            Action::Round(round) => shape.is_some_and(|shape| match shape.tool {
                Tool::Spotlight => shape.filled == round,
                _ => shape.filled != round,
            }),
            Action::BlurStyle(style) =>shape.is_some_and(|shape| shape.tool == Tool::Blur && (shape.style == effects::SMOOTH) == (style == effects::SMOOTH)),
            Action::FlipAcross => matches!(shown, Some(Layer { kind: Kind::Picture(picture), .. }) if picture.flip.0),
            Action::FlipDown => matches!(shown, Some(Layer { kind: Kind::Picture(picture), .. }) if picture.flip.1),
            _ => false,
        }
    }

    /// What a field that opens a list says: the choice there is now, with
    /// a picture of it, and a word or two after.
    pub(super) fn menu_value(&mut self, kind: MenuKind) -> (menu::Look, String, String) {
        let shape = self.picked().and_then(|index| match &self.doc.layers[index].kind {
            Kind::Shape(shape) => Some(shape.clone()),
            Kind::Picture(_) => None,
        });
        match kind {
            MenuKind::Fonts => {
                let face = shape.map_or(self.face, |shape| shape.face);
                (menu::Look::Face(face), FACES[face].to_owned(), String::new())
            }
            MenuKind::Heads(end) => match shape {
                Some(shape) if shape.heads[end] => (menu::Look::Head(shape.ends[end]), shapes::HEADS[usize::from(shape.ends[end]).min(shapes::HEADS.len() - 1)].to_owned(), String::new()),
                _ => (menu::Look::Head(shapes::HEADS.len() as u8), "None".to_owned(), String::new()),
            },
            MenuKind::Sizes => {
                let size = (self.doc.width, self.doc.height);
                let name = self.frame_sizes.iter().find(|(_, w, h, _)| (*w, *h) == size).map(|(name, ..)| name.clone()).or_else(|| menu::SIZES.iter().find(|(_, _, w, h)| (*w, *h) == size).map(|(_, name, ..)| name.to_string())).unwrap_or_else(|| "Custom".to_owned());
                (menu::Look::Plain, name, format!("{} \u{d7} {}", self.doc.width, self.doc.height))
            }
            MenuKind::Edge => (menu::Look::Plain, menu::EDGES[usize::from(shape.map_or(0, |shape| shape.edge)).min(2)].to_owned(), String::new()),
            MenuKind::Dash => (menu::Look::Plain, menu::DASHES[usize::from(shape.as_ref().map_or(0, menu::dash_of))].to_owned(), String::new()),
            MenuKind::Hand => (menu::Look::Plain, if shape.is_some_and(|shape| shape.sketch) { "By hand" } else { "Clean" }.to_owned(), String::new()),
            MenuKind::Window => {
                let window = self.picked().and_then(|index| match &self.doc.layers[index].kind {
                    Kind::Picture(picture) => picture.chrome.as_ref().map(|chrome| chrome.style),
                    Kind::Shape(_) => None,
                });
                (menu::Look::Plain, window.unwrap_or_default().long().to_owned(), String::new())
            }
            MenuKind::Shadows => {
                let (preset, drop) = match self.picked().map(|index| &self.doc.layers[index]) {
                    Some(layer @ Layer { kind: Kind::Picture(picture), .. }) => (picture.shadow, layer.drop()),
                    Some(layer) => (layer.drop().and_then(|drop| (1..=frame::SHADOWS.len()).find(|&which| actions::custom_drop(which) == drop)).unwrap_or(CUSTOM_SHADOW), layer.drop()),
                    None => (0, None),
                };
                let hint = match preset {
                    _ if drop.is_none() => String::new(),
                    which if which >= 1 && which <= frame::SHADOWS.len() => frame::SHADOWS[which - 1].0.to_owned(),
                    _ => "custom".to_owned(),
                };
                (menu::Look::Plain, "Drop shadow".to_owned(), hint)
            }
            MenuKind::Format(index) => (menu::Look::Plain, self.doc.exports.get(index).map_or("PNG", |size| size.format.family()).to_owned(), String::new()),
            _ => (menu::Look::Plain, String::new(), String::new()),
        }
    }
}

impl Studio {
    /// The frame dimmed but where some layers are, and those ringed.
    pub(super) fn dim_except(&mut self, canvas: &mut Canvas, layers: &[usize]) {
        let frame = self.frame_on_screen();
        let (a, b) = ((frame.x, frame.y), (frame.right(), frame.bottom()));
        let count = self.doc.layers.len();
        let holes: Vec<Rect> = layers
            .iter()
            .filter(|&&index| index < count)
            .map(|&index| {
                let (x, y, w, h) = self.doc.layers[index].bounds(&mut self.text);
                let (p, q) = (self.to_window((x, y)), self.to_window((x + w, y + h)));
                Rect::new(p.0, p.1, q.0 - p.0, q.1 - p.1).inflate(4)
            })
            .collect();
        for y in a.1.max(0)..b.1.min(canvas.height) {
            let mut x = a.0.max(0);
            let end = b.0.min(canvas.width);
            while x < end {
                match holes.iter().filter(|hole| y >= hole.y && y < hole.bottom() && x < hole.right() && hole.right() > x && hole.x <= x).map(|hole| hole.right()).max() {
                    Some(past) => x = past,
                    None => {
                        let next = holes.iter().filter(|hole| y >= hole.y && y < hole.bottom() && hole.x > x).map(|hole| hole.x).min().unwrap_or(end).min(end);
                        canvas.round_fill(x, y, next - x, 1, 0, 0x1E1E1E, 150);
                        x = next;
                    }
                }
            }
        }
        for hole in holes {
            canvas.round_outline(hole.x, hole.y, hole.w, hole.h, 4, 2, ACCENT);
        }
    }
}

impl Studio {
    /// The sizes the frame worked in is exported at, each with what its
    /// file ends in and its kind, and a plus for another.
    pub(super) fn export_sizes(&mut self, rows: &mut Rows) {
        rows.section("Export", "", Some((Control::Action(Action::AddExport), icons::PLUS)));
        for index in 0..self.doc.exports.len().min(6) {
            rows.split(&[(Control::Field(Field::ExportTimes(index as u8)), 52), (Control::Entry(Naming::Suffix(index)), 70), (Control::Menu(MenuKind::Format(index)), 0), (Control::Action(Action::RemoveExport(index as u8)), FIELD_H)]);
            // Under it, what it writes, tried for real: how large, and
            // how much smaller than a PNG or how good.
            let (left, right, ink) = self.row_words(&self.doc.exports[index].clone());
            rows.layout.words.push(((rows.left + 2, rows.y - 2), left, DIM_TEXT_COLOR, 11));
            if !right.is_empty() {
                let wide = self.text.width(&right, 11);
                rows.layout.words.push(((rows.left + rows.inner - FIELD_H - 6 - wide, rows.y - 2), right, ink, 11));
            }
            rows.y += Text::line_height(11) + 6;
        }
        self.want_sizes();
    }
}

//! A picture's window: one of the windows `chrome` draws put round it on
//! the canvas, the window's own title bar and borders taken off under it,
//! from the picture's Window section, Ctrl+K or the list of them; and
//! where on the picture that bar and its window's contents are.

use super::*;

/// Which file the window chosen last on the canvas is kept in, for the
/// next window captured to be put in it too.
const KEPT: &str = "canvas-window";

/// The window chosen last on the canvas, if one has been: none in tests,
/// which never read or write what the user keeps.
pub(super) fn kept_window() -> Option<chrome::Style> {
    if cfg!(test) {
        return None;
    }
    let text = std::fs::read_to_string(crate::output::state_file(KEPT)?).ok()?;
    chrome::Style::named(text.trim())
}

fn keep_window(style: chrome::Style) {
    if cfg!(test) {
        return;
    }
    let Some(file) = crate::output::state_file(KEPT) else { return };
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(file, format!("{}\n", style.word()));
}

impl Picture {
    /// The window drawn round it: none, without one or when its own bar
    /// is kept.
    pub(super) fn drawn_style(&self) -> chrome::Style {
        match &self.chrome {
            Some(chrome) if chrome.swap => chrome.style,
            _ => chrome::Style::None,
        }
    }

    /// The part of its own pixels shown: its crop, less the window's own
    /// title bar and borders where a window of hotshot's takes their place.
    pub(super) fn shown_crop(&self) -> (f32, f32, f32, f32) {
        let (x, y, w, h) = self.crop;
        let Some([l, r, t, b]) = self.insets.filter(|_| self.chrome.as_ref().is_some_and(|chrome| chrome.swap)) else { return self.crop };
        let (wide, high) = (self.image.width() as f32, self.image.height() as f32);
        let (left, top, right, bottom) = (x.max(l as f32), y.max(t as f32), (x + w).min(wide - r as f32), (y + h).min(high - b as f32));
        if right - left < 1.0 || bottom - top < 1.0 {
            return self.crop;
        }
        (left, top, right - left, bottom - top)
    }

    /// How many pixels of the artboard each of its own is shown across.
    pub(super) fn content_scale(&self) -> f32 {
        self.w / self.shown_crop().2.max(1e-3)
    }

    /// How tall the bar of its window is, in pixels of the artboard: as
    /// large against what is in it as such a bar was on its screen.
    pub(super) fn bar(&self) -> f32 {
        (self.drawn_style().bar() * self.screen_scale * self.content_scale()).min(self.h * 0.5)
    }

    /// How round its window's corners are, in pixels of the artboard.
    pub(super) fn window_radius(&self) -> f32 {
        self.drawn_style().radius() * self.screen_scale * self.content_scale()
    }

    /// The shadow the system gives its window, in pixels of its screen.
    pub(super) fn system_shadow(&self) -> &'static [frame::Layer] {
        self.chrome.as_ref().map_or(&[], |chrome| chrome.style.shadow())
    }

    /// Put in a window, or another, or none: as large as it was against
    /// its own pixels, which stay where they were on the artboard, its
    /// own bar coming off and the new one going on over where it was.
    pub(super) fn set_chrome(&mut self, chrome: Option<Chrome>) {
        let (was, bar) = (self.shown_crop(), self.bar());
        let (across, down) = (self.w / was.2.max(1e-3), (self.h - bar).max(1e-3) / was.3.max(1e-3));
        // Where what is in it begins, from its middle, unturned.
        let (left, top) = (-self.w / 2.0, -self.h / 2.0 + bar);
        self.chrome = chrome;
        let now = self.shown_crop();
        let bar = self.drawn_style().bar() * self.screen_scale * across;
        let (w, h) = (across * now.2, down * now.3 + bar);
        let dx = if self.flip.0 { (was.0 + was.2) - (now.0 + now.2) } else { now.0 - was.0 } * across;
        let dy = if self.flip.1 { (was.1 + was.3) - (now.1 + now.3) } else { now.1 - was.1 } * down;
        let middle = self.placed((left + dx + w / 2.0, top + dy - bar + h / 2.0));
        (self.cx, self.cy, self.w, self.h) = (middle.0, middle.1, w, h);
    }

    /// Where its window's own title bar is, looked for in its pixels when
    /// nothing said so as it was taken: once, and nothing if it has none.
    pub(super) fn find_insets(&mut self) {
        if self.insets.is_none() {
            let (w, h, pixels) = &self.image.levels[0];
            self.insets = Some([0, 0, edges::title_bar(*w, *h, pixels).unwrap_or(0), 0]);
        }
    }
}

/// What the chips of the Theme row say, and the theme each is.
pub(super) const THEMES: [chrome::Theme; 3] = [chrome::Theme::Light, chrome::Theme::Dark, chrome::Theme::Auto];

impl Studio {
    /// A change to the window of each picture picked out, one step to undo:
    /// a window given to one that had none, the settings it is to have.
    fn rechrome(&mut self, change: impl Fn(&mut Chrome)) {
        self.restyle(|layer| {
            if let Kind::Picture(picture) = &mut layer.kind {
                picture.find_insets();
                let mut chrome = picture.chrome.clone().unwrap_or_else(|| Chrome::new(chrome::Style::None));
                change(&mut chrome);
                picture.set_chrome(Some(chrome));
            }
        });
    }

    /// One of the windows put round what is picked out, and kept for the
    /// next window captured.
    pub(super) fn put_window(&mut self, style: chrome::Style) {
        self.rechrome(|chrome| chrome.style = style);
        if self.preview.is_none() && self.chosen().iter().any(|&index| matches!(self.doc.layers[index].kind, Kind::Picture(_))) {
            keep_window(style);
        }
    }

    pub(super) fn put_window_theme(&mut self, theme: chrome::Theme) {
        self.rechrome(|chrome| chrome.theme = theme);
    }

    pub(super) fn put_window_bar(&mut self, swap: bool) {
        self.rechrome(|chrome| chrome.swap = swap);
    }

    pub(super) fn put_window_shadow(&mut self, shadow: u8) {
        self.rechrome(|chrome| chrome.shadow = shadow);
    }

    /// A picture just put on the canvas in the window chosen last, if it
    /// is of a window and one has been chosen.
    pub(super) fn window_as_last(&mut self, index: usize) {
        let Some(style) = kept_window() else { return };
        if let Some(Layer { kind: Kind::Picture(picture), .. }) = self.doc.layers.get_mut(index) {
            if picture.insets.is_some_and(|insets| insets.iter().any(|&side| side > 0)) && picture.chrome.is_none() {
                picture.set_chrome(Some(Chrome::new(style)));
            }
        }
    }

    /// The picture being cropped is cropped without its window, all of
    /// its own pixels to be seen, its own bar too; and has it back once
    /// the crop is done. Kept in step with what is cropped whenever the
    /// pointer or a key comes, and before anything is drawn.
    pub(super) fn crop_window(&mut self) {
        let cropping = self.cropping.and_then(|index| match self.doc.layers.get(index).map(|layer| &layer.kind) {
            Some(Kind::Picture(picture)) => Some((index, picture.id)),
            _ => None,
        });
        if let Some((id, chrome)) = self.crop_chrome.clone() {
            if cropping.map(|(_, cropped)| cropped) == Some(id) {
                return;
            }
            self.crop_chrome = None;
            if let Some(index) = self.layer_by_id(id) {
                if let Kind::Picture(picture) = &mut self.doc.layers[index].kind {
                    if picture.chrome.is_none() {
                        picture.set_chrome(Some(chrome));
                        self.changed();
                    }
                }
            }
        }
        if let Some((index, id)) = cropping {
            if let Kind::Picture(picture) = &mut self.doc.layers[index].kind {
                if let Some(chrome) = picture.chrome.clone() {
                    picture.set_chrome(None);
                    self.crop_chrome = Some((id, chrome));
                    self.changed();
                }
            }
        }
    }

    /// The pages with the picture being cropped in its window still, as
    /// it is to be kept and come back to.
    pub(super) fn put_back_crop_window(&self, pages: &mut [Page]) {
        let Some((id, chrome)) = &self.crop_chrome else { return };
        let Some(frame) = pages.get_mut(self.page).and_then(|page| match self.active {
            LOOSE => Some(&mut page.loose),
            active => page.frames.get_mut(active),
        }) else {
            return;
        };
        for layer in &mut frame.layers {
            if let Kind::Picture(picture) = &mut layer.kind {
                if picture.id == *id && picture.chrome.is_none() {
                    picture.set_chrome(Some(chrome.clone()));
                }
            }
        }
    }

    /// The Window section of a picture's panel: the windows there are, as
    /// chips; light, dark or as its contents are; its title, and a
    /// browser's address; its own bar taken off or kept; and its shadow.
    pub(super) fn window_section(&mut self, rows: &mut panel::Rows, index: usize, picture: &Picture) {
        rows.section("Window", "Picture", None);
        let chips: Vec<(String, Control)> = chrome::CHOICES.iter().enumerate().map(|(at, style)| (style.says().to_owned(), Control::Window(at as u8))).collect();
        rows.grid(&mut self.text, chips, 4);
        let themes = THEMES.iter().enumerate().map(|(at, theme)| (theme.says().to_owned(), Control::WindowTheme(at as u8))).collect();
        rows.labeled(&mut self.text, "Theme", themes);
        rows.labeled_wide("Title", Control::Entry(Naming::Title(index)));
        if picture.chrome.as_ref().is_some_and(|chrome| chrome.style == chrome::Style::Browser) {
            rows.labeled_wide("Address", Control::Entry(Naming::Address(index)));
        }
        rows.labeled(&mut self.text, "Bar", vec![("Swap".into(), Control::WindowBar(true)), ("Keep own".into(), Control::WindowBar(false))]);
        let shadows = ["System", "Off", "Custom"].iter().enumerate().map(|(at, says)| ((*says).to_owned(), Control::WindowShadow(at as u8))).collect();
        rows.labeled(&mut self.text, "Shadow", shadows);
        if picture.chrome.as_ref().is_some_and(|chrome| !chrome.swap) {
            rows.note("Its own bar stays, and no window is drawn");
        } else if picture.insets.is_some_and(|insets| insets.iter().all(|&side| side == 0)) && picture.chrome.is_some() {
            rows.note("No bar of its own was found to take off");
        }
    }

    /// Whether a chip of the Window section is what the picture has.
    pub(super) fn window_chip_on(&self, control: Control, shown: &Option<Layer>) -> bool {
        let Some(Layer { kind: Kind::Picture(picture), .. }) = shown else { return false };
        let chrome = picture.chrome.as_ref();
        match control {
            Control::Window(which) => {
                let style = chrome::CHOICES[usize::from(which).min(chrome::CHOICES.len() - 1)];
                chrome.map_or(chrome::Style::None, |chrome| chrome.style) == style
            }
            Control::WindowTheme(which) => chrome.is_some_and(|chrome| chrome.theme == THEMES[usize::from(which).min(2)]),
            Control::WindowBar(swap) => chrome.is_some_and(|chrome| chrome.swap == swap),
            Control::WindowShadow(which) => chrome.is_some_and(|chrome| chrome.shadow == which),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A capture of a window: a dark bar 24 high over a light window, with
    /// what it knows of where its bar is.
    fn window_capture() -> Image {
        let (w, h) = (400, 300);
        let mut pixels = vec![0xFFF4_F4F5u32; (w * h) as usize];
        for y in 0..24 {
            for x in 0..w {
                pixels[(y * w + x) as usize] = 0xFF3C_3F41;
            }
        }
        let png = crate::output::encode(w as u32, h as u32, &pixels, true).unwrap();
        let notes = chrome::Notes { insets: Some([0, 0, 24, 0]), title: "Settings \u{2014} Acme".into(), scale: 1.0 };
        Image::open(&crate::output::noted(png, &notes.write())).unwrap()
    }

    fn studio() -> Studio {
        let mut studio = Studio::new(Doc::new(800, 600), Text::load(), 1400, 900, std::env::temp_dir(), "test-%Y.png".into());
        (studio.zoom, studio.pan) = (1.0, (300.0, 100.0));
        studio
    }

    fn picture(studio: &Studio) -> Picture {
        match &studio.doc.layers[0].kind {
            Kind::Picture(picture) => picture.clone(),
            _ => panic!("a picture"),
        }
    }

    #[test]
    fn a_window_capture_knows_its_bar_and_title_and_a_window_swaps_the_bar() {
        let mut studio = studio();
        studio.add_picture(window_capture(), "Capture");
        let before = picture(&studio);
        assert_eq!((before.insets, before.title.as_str()), (Some([0, 0, 24, 0]), "Settings \u{2014} Acme"));
        let (top, left) = (before.cy - before.h / 2.0, before.cx - before.w / 2.0);
        // macOS round it: its own bar off, the Mac's 28 on in its place,
        // what is in the window where it was.
        studio.put_window(chrome::Style::Mac);
        let mac = picture(&studio);
        assert_eq!(mac.shown_crop(), (0.0, 24.0, 400.0, 276.0));
        assert!((mac.bar() - 28.0).abs() < 0.01);
        assert!((mac.w - 400.0).abs() < 0.01 && (mac.h - (276.0 + 28.0)).abs() < 0.01, "{} {}", mac.w, mac.h);
        assert!((mac.cy - mac.h / 2.0 - (top + 24.0 - 28.0)).abs() < 0.01 && (mac.cx - mac.w / 2.0 - left).abs() < 0.01);
        // Drawn: the lights at the left of its bar, its contents under it.
        let (wide, _, pixels) = studio.doc.render(1.0, &mut studio.text);
        let at = |x: f32, y: f32| pixels[(y as i32 * wide + x as i32) as usize] & 0xFF_FFFF;
        let (x0, y0) = (mac.cx - mac.w / 2.0, mac.cy - mac.h / 2.0);
        assert_eq!(at(x0 + 15.5, y0 + 14.0), 0xFF5F57);
        assert_eq!(at(x0 + 200.0, y0 + 40.0), 0xF4F4F5);
        // Kept own: the bar it had, nothing drawn; and back.
        studio.put_window_bar(false);
        let kept = picture(&studio);
        assert_eq!((kept.bar(), kept.shown_crop()), (0.0, (0.0, 0.0, 400.0, 300.0)));
        assert!((kept.cy - kept.h / 2.0 - top).abs() < 0.01);
        studio.put_window_bar(true);
        // Each window is another bar; one undo for each, back to none.
        for style in chrome::CHOICES {
            studio.put_window(style);
            assert_eq!(picture(&studio).drawn_style(), style);
        }
        for _ in 0..chrome::CHOICES.len() + 3 {
            studio.key(Key::Char('z'), Mods { ctrl: true, shift: false, alt: false }, true);
        }
        let back = picture(&studio);
        assert!(back.chrome.is_none() && (back.h - 300.0).abs() < 0.01);
    }

    #[test]
    fn ctrl_k_puts_a_window_round_what_is_picked_out_and_the_panel_has_its_chips() {
        let mut studio = studio();
        studio.add_picture(window_capture(), "Capture");
        studio.key(Key::Char('k'), Mods { ctrl: true, shift: false, alt: false }, true);
        for letter in "gnome".chars() {
            studio.key(Key::Char(letter), Mods::default(), true);
        }
        assert_eq!(studio.hits().first().map(|hit| hit.says.clone()).as_deref(), Some("Window: GNOME (Adwaita)"));
        studio.key(Key::Enter, Mods::default(), true);
        assert_eq!(picture(&studio).drawn_style(), chrome::Style::Gnome);
        // The panel: eight chips, the one it has lit; hovered, another
        // shows on the canvas, and only a click keeps it.
        let layout = studio.layout();
        let chips: Vec<(Rect, Control)> = layout.controls.iter().filter(|(_, control)| matches!(control, Control::Window(_))).copied().collect();
        assert_eq!(chips.len(), 8);
        let shown = Some(studio.doc.layers[0].clone());
        assert!(studio.window_chip_on(Control::Window(2), &shown) && !studio.window_chip_on(Control::Window(0), &shown));
        let (rect, _) = chips[1];
        studio.pointer_moved(rect.center().0, rect.center().1, Mods::default());
        assert_eq!(picture(&studio).drawn_style(), chrome::Style::Windows);
        studio.pointer_moved(rect.x - 300, rect.y, Mods::default());
        assert_eq!(picture(&studio).drawn_style(), chrome::Style::Gnome);
    }

    #[test]
    fn a_window_is_kept_in_the_file_and_an_old_files_window_is_read() {
        let mut studio = studio();
        studio.add_picture(window_capture(), "Capture");
        studio.put_window(chrome::Style::Browser);
        studio.put_window_theme(chrome::Theme::Dark);
        if let Kind::Picture(picture) = &mut studio.doc.layers[0].kind {
            picture.chrome.as_mut().unwrap().address = "acme.app/settings".into();
        }
        let read = Doc::read(&studio.doc.write()).unwrap();
        let Kind::Picture(back) = &read.layers[0].kind else { panic!() };
        let now = picture(&studio);
        assert_eq!((back.chrome.clone(), back.insets, back.title.clone()), (now.chrome.clone(), now.insets, now.title.clone()));
        assert!((back.h - now.h).abs() < 0.01);
        // A file from before had a window by its number, 2 a Mac's.
        let mut doc = Doc::new(100, 100);
        let line = format!("picture 0 0 0 50 50 100 100 0 0 0 100 100 0 0 256 0 0 7 pic");
        let image = Arc::new(Image::new(100, 100, vec![0xFFFF_FFFF; 100 * 100]));
        doc.read_line(&line, &[image]).unwrap();
        doc.read_line("window 2 main.rs", &[]).unwrap();
        let Kind::Picture(old) = &doc.layers[0].kind else { panic!() };
        assert_eq!((old.chrome.as_ref().map(|chrome| chrome.style), old.title.as_str()), (Some(chrome::Style::Mac), "main.rs"));
    }

    #[test]
    fn a_pasted_window_has_its_bar_found_and_cropping_shows_all_of_it() {
        let mut studio = studio();
        // The same capture with nothing said of it.
        let plain = window_capture();
        let (w, h, pixels) = plain.levels[0].clone();
        studio.add_picture(Image::new(w, h, pixels), "Pasted");
        assert_eq!(picture(&studio).insets, None);
        studio.put_window(chrome::Style::Gnome);
        assert_eq!(picture(&studio).insets, Some([0, 0, 24, 0]));
        let framed = picture(&studio);
        // Cropped, it is all of its pixels, its own bar too, and in its
        // window again after; what is kept meanwhile is in its window.
        studio.cropping = Some(0);
        studio.crop_window();
        assert!(picture(&studio).chrome.is_none() && (picture(&studio).h - 300.0).abs() < 0.01);
        let kept = studio.gathered();
        let Kind::Picture(kept) = &kept[0].frames[0].layers[0].kind else { panic!() };
        assert!(kept.chrome.is_some());
        studio.cropping = None;
        studio.crop_window();
        let after = picture(&studio);
        assert_eq!(after.chrome, framed.chrome);
        assert!((after.h - framed.h).abs() < 0.01 && (after.cy - framed.cy).abs() < 0.01);
    }
}

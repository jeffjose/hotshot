//! Ctrl+K remembers: what was done lately, with what to, is listed first
//! when it is opened with nothing typed, the most recent first, then what
//! is done most; typing ranks what it finds the same way; and Ctrl+Shift+K
//! does the last again to what is picked out now. What was done, and how
//! often, is kept among hotshot's state, the last fifty, never in a file
//! of the canvas.

use super::*;
use menu::MenuAct;

/// How many are kept.
const KEPT: usize = 50;
/// How many are listed as done lately, and as done most.
const RECENT: usize = 4;
const OFTEN: usize = 3;

/// Something done: what it says, how many times it has been, and when it
/// last was, in seconds since 1970.
#[derive(Clone, PartialEq, Debug)]
pub(super) struct Done {
    pub says: String,
    pub count: u32,
    pub when: u64,
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |since| since.as_secs())
}

/// The file they are kept in, among hotshot's state.
pub(super) fn file() -> Option<PathBuf> {
    crate::output::state_file("commands")
}

/// What a file of them says: a line each, when, how many times, and what.
pub(super) fn read(text: &str) -> Vec<Done> {
    text.lines()
        .filter_map(|line| {
            let mut words = line.splitn(3, ' ');
            let when = words.next()?.parse().ok()?;
            let count = words.next()?.parse().ok()?;
            let says = words.next()?.trim();
            (!says.is_empty()).then(|| Done { says: says.to_owned(), count, when })
        })
        .take(KEPT)
        .collect()
}

pub(super) fn write(done: &[Done]) -> String {
    done.iter().map(|done| format!("{} {} {}\n", done.when, done.count, done.says.replace('\n', " "))).collect()
}

/// How long ago something was, said shortly.
pub(super) fn ago(when: u64, now: u64) -> String {
    let past = now.saturating_sub(when);
    match past {
        0..60 => "just now".to_owned(),
        60..3600 => format!("{} min", past / 60),
        3600..86400 => format!("{} h", past / 3600),
        _ => format!("{} d", past / 86400),
    }
}

impl Studio {
    /// What is kept read, from where it is kept; nothing in a test, which
    /// keeps what it does to itself.
    pub(super) fn recall_commands(&mut self) {
        self.commands_file = file();
        self.commands = self.commands_file.as_ref().and_then(|file| std::fs::read_to_string(file).ok()).map(|text| read(&text)).unwrap_or_default();
    }

    /// What something done says in the search, if it is something the
    /// search lists now: asked before it is done, as doing it may change
    /// what is picked out.
    pub(super) fn says_of(&mut self, act: MenuAct) -> Option<String> {
        if self.preview.is_some() {
            return None;
        }
        self.acts().into_iter().find(|(_, _, known)| *known == act).map(|(says, ..)| says)
    }

    /// Something just done remembered: moved to the top, once more.
    pub(super) fn remember(&mut self, says: Option<String>) {
        let Some(says) = says.filter(|_| self.preview.is_none()) else { return };
        let count = match self.commands.iter().position(|done| done.says == says) {
            Some(at) => self.commands.remove(at).count + 1,
            None => 1,
        };
        self.commands.insert(0, Done { says, count, when: now() });
        self.commands.truncate(KEPT);
        if let Some(file) = &self.commands_file {
            if let Some(dir) = file.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(file, write(&self.commands));
        }
    }

    /// How much what something says was used: lately and often, for what
    /// is found to be ranked by. Nothing for what never was.
    pub(super) fn used(&self, says: &str) -> f32 {
        let now = now();
        self.commands.iter().position(|done| done.says == says).map_or(0.0, |at| {
            let done = &self.commands[at];
            let minutes = now.saturating_sub(done.when) as f32 / 60.0;
            done.count as f32 + 40.0 / (1.0 + minutes / 10.0) + (KEPT - at) as f32 * 0.1
        })
    }

    /// The rows the search opens with, nothing typed: what was done lately,
    /// with how long ago, the last saying Ctrl+Shift+K does it again; and
    /// what is done most, with its keys and how many times. Those that can
    /// be done now, each once.
    pub(super) fn remembered_hits(&mut self) -> Vec<search::Hit> {
        let acts = self.acts();
        let find = |says: &str| acts.iter().find(|(known, ..)| known == says);
        let now = now();
        let mut hits = Vec::new();
        for (at, done) in self.commands.iter().filter(|done| find(&done.says).is_some()).take(RECENT).enumerate() {
            let Some((_, _, act)) = find(&done.says) else { continue };
            let when = ago(done.when, now);
            let hint = if at == 0 { format!("{when} \u{b7} Ctrl \u{21e7} K again") } else { when };
            hits.push(search::Hit { heading: "RECENT", says: done.says.clone(), hint, found: search::Found::Act(*act) });
        }
        let mut often: Vec<&Done> = self.commands.iter().filter(|done| done.count > 1 && find(&done.says).is_some() && !hits.iter().any(|hit| hit.says == done.says)).collect();
        often.sort_by_key(|done| std::cmp::Reverse(done.count));
        for done in often.into_iter().take(OFTEN) {
            let Some((_, keys, act)) = find(&done.says) else { continue };
            let hint = if keys.is_empty() { format!("{} times", done.count) } else { format!("{keys} \u{b7} {} times", done.count) };
            hits.push(search::Hit { heading: "OFTEN", says: done.says.clone(), hint, found: search::Found::Act(*act) });
        }
        hits
    }

    /// Ctrl+Shift+K: the last thing done done again, to what is picked out
    /// now, as one step to undo; what it did said along the bottom.
    pub(super) fn again(&mut self) -> Outcome {
        let Some(last) = self.commands.first().cloned() else {
            self.say("Nothing done yet to do again");
            return Outcome::Continue;
        };
        let Some((_, _, act)) = self.acts().into_iter().find(|(says, ..)| *says == last.says) else {
            self.say(format!("{} cannot be done to this", last.says));
            return Outcome::Continue;
        };
        let before = self.undo.len();
        let outcome = self.run_menu(act);
        // However many steps it took, one to undo.
        if self.undo.len() > before + 1 {
            let first = self.undo[before].clone();
            self.undo.truncate(before);
            self.undo.push(first);
        }
        self.say(format!("Again: {}", last.says));
        outcome
    }

    /// What the panel's buttons do to what is picked out, by name, to be
    /// found and done again: lining up, spacing, matching, putting to the
    /// frame and turning.
    pub(super) fn named_actions(&self) -> Vec<(String, &'static str, MenuAct)> {
        let chosen = self.chosen();
        if chosen.is_empty() {
            return Vec::new();
        }
        let mut named: Vec<(&str, &'static str, Action)> = vec![
            ("Align left", "", Action::Align(0)),
            ("Align centers", "", Action::Align(1)),
            ("Align right", "", Action::Align(2)),
            ("Align top", "", Action::Align(3)),
            ("Align middles", "", Action::Align(4)),
            ("Align bottom", "", Action::Align(5)),
            ("Turn left 90\u{b0}", "", Action::Turn(-90)),
            ("Turn right 90\u{b0}", "", Action::Turn(90)),
            ("Turn 180\u{b0}", "", Action::Turn(180)),
            ("Center on the frame", "", Action::ToFrame(6)),
        ];
        if chosen.len() > 1 {
            named.extend([
                ("Equal gaps across", "", Action::Distribute(0)),
                ("Equal gaps down", "", Action::Distribute(1)),
                ("Pack across", "", Action::Pack(0)),
                ("Pack down", "", Action::Pack(1)),
                ("Match width", "", Action::Match(0)),
                ("Match height", "", Action::Match(1)),
                ("Match size", "", Action::Match(2)),
                ("Match corners", "", Action::Match(3)),
                ("Match shadow", "", Action::Match(4)),
                ("Match whole look", "", Action::Match(5)),
            ]);
        }
        named.into_iter().map(|(says, keys, action)| (says.to_owned(), keys, MenuAct::Act(action))).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_was_done_is_written_down_and_read_back_and_said_as_how_long_ago() {
        let done = vec![Done { says: "Tight crop".into(), count: 3, when: 1000 }, Done { says: "Window: macOS".into(), count: 1, when: 900 }];
        assert_eq!(read(&write(&done)), done);
        assert_eq!((ago(1000, 1010), ago(1000, 1000 + 120), ago(1000, 1000 + 7200), ago(1000, 1000 + 86400 * 3)), ("just now".into(), "2 min".into(), "2 h".into(), "3 d".into()));
    }

    fn studio() -> Studio {
        let mut studio = Studio::new(Doc::new(800, 600), Text::load(), 1400, 900, std::env::temp_dir(), "test-%Y.png".into());
        (studio.zoom, studio.pan) = (1.0, (300.0, 100.0));
        studio
    }

    const CTRL: Mods = Mods { ctrl: true, shift: false, alt: false };

    fn put(studio: &mut Studio, (x, y): (f32, f32)) -> usize {
        let image = Arc::new(Image::new(40, 30, vec![0xFF3E63DD; 40 * 30]));
        studio.doc.layers.push(Layer::picture(Picture::new(image, "box".into(), (x, y))));
        studio.doc.layers.len() - 1
    }

    #[test]
    fn ctrl_k_lists_what_was_done_lately_and_most_and_ctrl_shift_k_does_the_last_again() {
        let mut studio = studio();
        assert!(studio.commands_file.is_none(), "a test keeps what it does to itself");
        let (a, b) = (put(&mut studio, (100.0, 100.0)), put(&mut studio, (300.0, 200.0)));
        // Done from the panel, the keys and the search: each remembered,
        // with what it was done to.
        studio.selected = vec![a, b];
        drop(studio.act(Action::Align(4)));
        studio.key(Key::Char('d'), CTRL, true);
        studio.selected = vec![a];
        studio.key(Key::Char('d'), CTRL, true);
        studio.selected = vec![a];
        studio.press_control(Control::Window(0), false);
        let says: Vec<(String, u32)> = studio.commands.iter().map(|done| (done.says.clone(), done.count)).collect();
        assert_eq!(says, [("Window: macOS".to_owned(), 1), ("Duplicate".to_owned(), 2), ("Align middles".to_owned(), 1)]);
        // Hovered, a chip is shown and not remembered.
        let chip = studio.control_rect(Control::Window(1)).expect("the chip is in the panel");
        studio.pointer_moved(chip.center().0, chip.center().1, Mods::default());
        studio.pointer_moved(10, 10, Mods::default());
        assert_eq!(studio.commands.len(), 3);
        // Opened with nothing typed: what was done lately, then most, then
        // the tools.
        studio.key(Key::Char('k'), CTRL, true);
        let hits = studio.hits();
        let headings: Vec<&str> = hits.iter().map(|hit| hit.heading).take(4).collect();
        assert_eq!(headings, ["RECENT", "RECENT", "RECENT", "TOOLS"]);
        assert_eq!((hits[0].says.as_str(), hits[0].hint.as_str()), ("Window: macOS", "just now \u{b7} Ctrl \u{21e7} K again"));
        // Typed, what was used comes before what was not.
        for letter in "align".chars() {
            studio.key(Key::Char(letter), Mods::default(), true);
        }
        assert_eq!(studio.hits()[0].says, "Align middles");
        studio.key(Key::Escape, Mods::default(), true);
        // Ctrl+Shift+K: the last again, to what is picked out now; one
        // step to undo, and said.
        let other = studio.doc.layers.len() - 1;
        studio.selected = vec![other];
        let undo = studio.undo.len();
        studio.key(Key::Char('K'), Mods { ctrl: true, shift: true, alt: false }, true);
        assert!(matches!(&studio.doc.layers[other].kind, Kind::Picture(picture) if picture.chrome.as_ref().is_some_and(|chrome| chrome.style == chrome::Style::Mac)));
        assert_eq!(studio.undo.len(), undo + 1);
        assert_eq!(studio.note.as_ref().map(|(says, _)| says.as_str()), Some("Again: Window: macOS"));
        assert_eq!(studio.commands[0].count, 2);
        // With a duplicate last, Ctrl+Shift+K duplicates again.
        studio.key(Key::Char('d'), CTRL, true);
        let count = studio.doc.layers.len();
        studio.key(Key::Char('K'), Mods { ctrl: true, shift: true, alt: false }, true);
        assert_eq!(studio.doc.layers.len(), count + 1);
    }
}

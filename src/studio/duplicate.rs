//! Copies, as Figma makes them: the first beside what it is a copy of,
//! not on it; and the next copy of a copy that has been moved goes as far
//! again from it, so that a row or a grid lays itself out.

use super::*;
use crate::spacing::{self, Edges};

/// Between a frame and the first copy of it on the page, as Figma has it.
const FRAME_GAP: f32 = 100.0;
/// Between a layer and the first copy of it in its frame.
const LAYER_GAP: f32 = 20.0;

/// What a layer is called, which no copy of it is.
fn id_of(layer: &Layer) -> u32 {
    match &layer.kind {
        Kind::Picture(picture) => picture.id,
        Kind::Shape(shape) => shape.id,
    }
}

impl Studio {
    /// Whether what is picked out is the last copy made, and only that.
    pub(super) fn is_last_copy(&self, repeat: &Repeat) -> bool {
        match repeat {
            Repeat::Frame { page, copy, .. } => *page == self.page && *copy == self.active && self.frame_picked && self.chosen().is_empty(),
            Repeat::Layers { frame, copies, .. } => {
                let mut ids: Vec<u32> = self.chosen().iter().map(|&index| id_of(&self.doc.layers[index])).collect();
                let mut copies = copies.clone();
                ids.sort_unstable();
                copies.sort_unstable();
                *frame == (self.page, self.active) && !ids.is_empty() && ids == copies
            }
        }
    }

    /// The box round the layers called these, if they are all still there.
    fn box_of(&mut self, ids: &[u32]) -> Option<Edges> {
        let mut all: Option<Edges> = None;
        for &id in ids {
            let (x, y, w, h) = self.doc.layers[self.layer_by_id(id)?].bounds(&mut self.text);
            all = Some(all.map_or([x, y, x + w, y + h], |was| [was[0].min(x), was[1].min(y), was[2].max(x + w), was[3].max(y + h)]));
        }
        all
    }

    /// Ctrl+D: what is picked out copied, or the frame if it is what is
    /// picked out. The last copy made, picked out, is copied as far from
    /// itself as it is from what it was copied from; anything else, beside
    /// itself, to the right or else below, where nothing else is.
    pub(super) fn duplicate(&mut self) {
        let chosen = self.chosen();
        if chosen.is_empty() {
            if self.frame_picked {
                self.duplicate_frame();
            }
            return;
        }
        let again = match self.repeat.clone().filter(|repeat| self.is_last_copy(repeat)) {
            Some(Repeat::Layers { sources, copies, .. }) => self.box_of(&sources).zip(self.box_of(&copies)).map(|(from, to)| (to[0] - from[0], to[1] - from[1])),
            _ => None,
        };
        let by = match again {
            Some(by) => by,
            None => {
                let ids: Vec<u32> = chosen.iter().map(|&index| id_of(&self.doc.layers[index])).collect();
                let me = self.box_of(&ids).unwrap_or_default();
                let others = self.other_edges();
                // Kept to the frame; on the page, kept to nothing.
                let within = (!self.on_page()).then_some([0.0, 0.0, self.doc.width as f32, self.doc.height as f32]);
                spacing::beside(me, &others, within, LAYER_GAP)
            }
        };
        self.copy_layers_by(by);
    }

    /// What is picked out copied, the copies so far from it and picked out
    /// instead; and remembered, for the next copy of them. One step to undo.
    pub(super) fn copy_layers_by(&mut self, by: (f32, f32)) {
        let chosen = self.chosen();
        let sources: Vec<u32> = chosen.iter().map(|&index| id_of(&self.doc.layers[index])).collect();
        self.checkpoint();
        self.selected.clear();
        let mut copies = Vec::new();
        for index in chosen {
            let mut copy = self.doc.layers[index].clone();
            match &mut copy.kind {
                Kind::Picture(picture) => picture.id = shapes::fresh_id(),
                Kind::Shape(shape) => *shape = shape.copied(),
            }
            copy.translate(by.0, by.1);
            copies.push(id_of(&copy));
            self.doc.layers.push(copy);
            self.selected.push(self.doc.layers.len() - 1);
        }
        // A copy of something in a stack goes next after it there, with
        // the stack's gap, rather than beside it.
        let pairs: Vec<(u32, u32)> = sources.iter().copied().zip(copies.iter().copied()).collect();
        self.stack_copies(&pairs);
        self.repeat = Some(Repeat::Layers { frame: (self.page, self.active), sources, copies });
        self.changed();
    }

    /// The frame worked in copied and the copy worked in: the last copy
    /// made as far again from itself, or anything else to the right of
    /// itself, 100 away and past any frame already there.
    pub(super) fn duplicate_frame(&mut self) {
        self.commit_name();
        self.commit_text();
        let again = match self.repeat.clone().filter(|repeat| self.is_last_copy(repeat)) {
            Some(Repeat::Frame { source, copy, .. }) if source < self.frame_count() => {
                let (from, to) = (self.frame(source), self.frame(copy));
                Some((2.0 * to.x - from.x, 2.0 * to.y - from.y))
            }
            _ => None,
        };
        let at = again.unwrap_or_else(|| {
            let me = [self.doc.x, self.doc.y, self.doc.x + self.doc.width as f32, self.doc.y + self.doc.height as f32];
            let (dx, dy) = spacing::beside(me, &self.other_frames(), None, FRAME_GAP);
            (self.doc.x + dx, self.doc.y + dy)
        });
        self.copy_frame(at);
    }

    /// The frame worked in copied to a place on the page, the copy worked
    /// in and remembered, for the next copy of it.
    pub(super) fn copy_frame(&mut self, at: (f32, f32)) {
        self.commit_name();
        self.commit_text();
        if self.on_page() {
            return self.say("Pick a frame to copy it");
        }
        let source = self.active;
        let mut copy = self.doc.clone();
        // A copy of a copy is called what it is, not "copy copy".
        if !copy.name.ends_with(" copy") {
            copy.name = format!("{} copy", copy.name);
        }
        (copy.x, copy.y) = at;
        self.checkpoint();
        self.pages[self.page].frames.push(copy);
        let index = self.frame_count() - 1;
        self.activate(index);
        self.frame_picked = true;
        self.repeat = Some(Repeat::Frame { page: self.page, source, copy: index });
        self.changed();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CTRL: Mods = Mods { ctrl: true, shift: false, alt: false };

    /// At 100%, for a press in the middle of a small box to be well away
    /// from where its corners turn it.
    fn studio() -> Studio {
        let mut studio = Studio::new(Doc::new(800, 600), Text::load(), 1400, 900, std::env::temp_dir(), "test-%Y.png".into());
        (studio.zoom, studio.pan) = (1.0, (300.0, 100.0));
        studio
    }

    /// A picture so large, its top left corner at a point of the frame.
    fn put(studio: &mut Studio, (x, y): (f32, f32), (w, h): (i32, i32)) -> usize {
        let image = Arc::new(Image::new(w, h, vec![0xFF3E63DD; (w * h) as usize]));
        studio.doc.layers.push(Layer::picture(Picture::new(image, "box".into(), (x + w as f32 / 2.0, y + h as f32 / 2.0))));
        studio.doc.layers.len() - 1
    }

    fn corner(studio: &mut Studio, index: usize) -> (f32, f32) {
        let (x, y, _, _) = studio.doc.layers[index].bounds(&mut studio.text);
        (x.round(), y.round())
    }

    /// A drag in the view from one point of the frame to another.
    fn drag(studio: &mut Studio, from: (f32, f32), to: (f32, f32), mods: Mods) {
        let (a, b) = (studio.to_window(from), studio.to_window(to));
        studio.pointer_moved(a.0, a.1, mods);
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        studio.pointer_moved((a.0 + b.0) / 2, (a.1 + b.1) / 2, mods);
        studio.pointer_moved(b.0, b.1, mods);
        studio.released();
    }

    #[test]
    fn a_layer_copied_goes_beside_itself_and_a_copy_moved_is_copied_as_far_again() {
        let mut studio = studio();
        let first = put(&mut studio, (100.0, 100.0), (50, 50));
        studio.selected = vec![first];
        // The first copy to the right, 20 away, and picked out.
        studio.key(Key::Char('d'), CTRL, true);
        assert_eq!((studio.doc.layers.len(), studio.selected.clone()), (2, vec![1]));
        assert_eq!(corner(&mut studio, 1), (170.0, 100.0));
        // That moved down 100 and copied: the next 70 across and 100 down
        // from it, as it is from the first; and again.
        drag(&mut studio, (190.0, 120.0), (190.0, 220.0), CTRL);
        assert_eq!(corner(&mut studio, 1), (170.0, 200.0));
        studio.key(Key::Char('d'), CTRL, true);
        assert_eq!(corner(&mut studio, 2), (240.0, 300.0));
        studio.key(Key::Char('d'), CTRL, true);
        assert_eq!(corner(&mut studio, 3), (310.0, 400.0));
        // Nudged by the keys, it still follows on.
        studio.key(Key::Right, Mods::default(), true);
        studio.key(Key::Char('d'), CTRL, true);
        assert_eq!(corner(&mut studio, 4), (382.0, 500.0));
        // One step to undo each.
        studio.key(Key::Char('z'), CTRL, true);
        studio.key(Key::Char('z'), CTRL, true);
        studio.key(Key::Char('z'), CTRL, true);
        assert_eq!(studio.doc.layers.len(), 3);
        studio.key(Key::Char('z'), CTRL, true);
        assert_eq!(studio.doc.layers.len(), 2);
        assert_eq!(corner(&mut studio, 1), (170.0, 200.0));
    }

    #[test]
    fn anything_else_done_forgets_the_last_copy() {
        let mut studio = studio();
        let first = put(&mut studio, (100.0, 100.0), (50, 50));
        studio.selected = vec![first];
        studio.key(Key::Char('d'), CTRL, true);
        drag(&mut studio, (190.0, 120.0), (190.0, 220.0), CTRL);
        // Put under the first: forgotten, so the next goes beside it.
        studio.restack(false);
        assert!(studio.repeat.is_none());
        studio.key(Key::Char('d'), CTRL, true);
        assert_eq!(corner(&mut studio, 2), (240.0, 200.0));
        // Another picked out: beside that, not as far again.
        studio.doc.layers.truncate(2);
        studio.selected = vec![1];
        studio.key(Key::Char('d'), CTRL, true);
        assert_eq!(corner(&mut studio, 1), (100.0, 100.0));
        assert_eq!(corner(&mut studio, 2), (170.0, 100.0));
    }

    #[test]
    fn a_copy_with_no_room_to_the_right_goes_below_and_several_go_together() {
        let mut studio = studio();
        // A picture filling the frame, not in the way of what is on it.
        put(&mut studio, (0.0, 0.0), (800, 600));
        let near = put(&mut studio, (700.0, 100.0), (60, 40));
        studio.selected = vec![near];
        studio.key(Key::Char('d'), CTRL, true);
        assert_eq!(corner(&mut studio, 2), (700.0, 160.0));
        // Two at once: the box round both beside itself, each as far.
        studio.doc.layers.truncate(2);
        let other = put(&mut studio, (100.0, 300.0), (50, 50));
        let more = put(&mut studio, (200.0, 320.0), (50, 50));
        studio.selected = vec![other, more];
        studio.key(Key::Char('d'), CTRL, true);
        assert_eq!((corner(&mut studio, 4), corner(&mut studio, 5)), ((270.0, 300.0), (370.0, 320.0)));
    }

    #[test]
    fn alt_dragged_a_layer_leaves_a_copy_behind_and_the_next_copy_follows_on() {
        let mut studio = studio();
        put(&mut studio, (100.0, 100.0), (50, 50));
        let alt = Mods { alt: true, ..CTRL };
        drag(&mut studio, (120.0, 120.0), (120.0, 220.0), alt);
        assert_eq!(studio.doc.layers.len(), 2);
        assert_eq!((corner(&mut studio, 0), corner(&mut studio, 1)), ((100.0, 100.0), (100.0, 200.0)));
        assert_eq!(studio.selected, vec![1]);
        // Ctrl+D goes on down.
        studio.key(Key::Char('d'), CTRL, true);
        assert_eq!(corner(&mut studio, 2), (100.0, 300.0));
        // The copy and its move are one step to undo.
        studio.key(Key::Char('z'), CTRL, true);
        studio.key(Key::Char('z'), CTRL, true);
        assert_eq!(studio.doc.layers.len(), 1);
        // Alt on nothing in the frame still pans.
        let pan = studio.pan;
        drag(&mut studio, (500.0, 500.0), (520.0, 500.0), Mods { alt: true, ..Mods::default() });
        assert_eq!(studio.doc.layers.len(), 1);
        assert!(studio.pan != pan);
    }

    /// A frame's name on the page dragged so far across the window.
    fn drag_frame(studio: &mut Studio, frame: usize, by: (i32, i32), mods: Mods) {
        let label = studio.frame_label(frame).0.center();
        studio.pointer_moved(label.0, label.1, mods);
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        studio.pointer_moved(label.0 + by.0 / 2, label.1 + by.1 / 2, mods);
        studio.pointer_moved(label.0 + by.0, label.1 + by.1, mods);
        studio.released();
    }

    #[test]
    fn a_frame_copied_goes_100_to_its_right_past_any_frame_there_and_a_moved_copy_follows_on() {
        let mut studio = studio();
        studio.new_frame((900.0, 50.0), (300.0, 200.0));
        studio.activate(0);
        studio.frame_picked = true;
        // Past the frame 100 to the right of it.
        studio.key(Key::Char('d'), CTRL, true);
        assert_eq!((studio.frame_count(), studio.active, studio.doc.x, studio.doc.y), (3, 2, 1300.0, 0.0));
        assert!(studio.doc.name.ends_with("copy"));
        // With room, 100 to the right; moved down by its name, and copied:
        // as far again.
        studio.undo();
        studio.pages[0].frames.remove(1);
        studio.frame_picked = true;
        studio.key(Key::Char('d'), CTRL, true);
        assert_eq!((studio.active, studio.doc.x), (1, 900.0));
        (studio.zoom, studio.pan) = (0.5, (50.0, 300.0));
        drag_frame(&mut studio, 1, (0, 40), CTRL);
        assert_eq!((studio.doc.x, studio.doc.y), (900.0, 80.0));
        studio.key(Key::Char('d'), CTRL, true);
        assert_eq!((studio.active, studio.doc.x, studio.doc.y), (2, 1800.0, 160.0));
        // Alt on its name drags off a copy, the frame left where it was;
        // the two one step to undo.
        drag_frame(&mut studio, 2, (40, 0), Mods { alt: true, ..CTRL });
        assert_eq!((studio.frame_count(), studio.active, studio.doc.x), (4, 3, 1880.0));
        assert_eq!(studio.frame(2).x, 1800.0);
        studio.undo();
        assert_eq!(studio.frame_count(), 3);
    }
}

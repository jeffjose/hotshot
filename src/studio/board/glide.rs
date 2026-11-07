//! The view of a board gliding over to something rather than jumping to
//! it: what was found, a link followed, a box made off the edge by the
//! keys. Easing out over a fifth of a second, the page drawn from what
//! was drawn last on the way, as it is while the wheel moves it, and
//! sharp once it is there. Moved by hand on the way, it stops there.

use super::*;
use std::time::{Duration, Instant};

/// How long a glide takes.
const GLIDE: Duration = Duration::from_millis(220);

/// How far in from the window's edges, and from under the find bar, what
/// is brought into view is kept.
const ROOM: f32 = 72.0;

/// A glide of the view: from where, to where, since when; and where it
/// put the view last, to tell a move by hand on the way.
#[derive(Clone, Copy, Debug)]
pub(in crate::studio) struct Glide {
    pub from: (f32, (f32, f32)),
    pub to: (f32, (f32, f32)),
    pub since: Instant,
    pub last: (f32, (f32, f32)),
}

impl Studio {
    /// The view glided over to a zoom and a pan.
    pub(in crate::studio) fn glide_to(&mut self, zoom: f32, pan: (f32, f32)) {
        let now = (self.zoom, self.pan);
        if (now.0 - zoom).abs() < 1e-4 && (now.1 .0 - pan.0).abs() < 0.5 && (now.1 .1 - pan.1).abs() < 0.5 {
            return;
        }
        self.board.glide = Some(Glide { from: now, to: (zoom, pan), since: Instant::now(), last: now });
        self.dirty = true;
    }

    /// Where the view is going: where it glides to, or where it is.
    pub(in crate::studio) fn view_to(&self) -> (f32, (f32, f32)) {
        self.board.glide.map_or((self.zoom, self.pan), |glide| glide.to)
    }

    /// The glide on a step, by the clock; there, or moved by hand, done.
    pub(in crate::studio) fn glide_tick(&mut self) {
        let Some(glide) = self.board.glide else { return };
        if (self.zoom, self.pan) != glide.last {
            self.board.glide = None;
            return;
        }
        let t = (glide.since.elapsed().as_secs_f32() / GLIDE.as_secs_f32()).min(1.0);
        let eased = 1.0 - (1.0 - t).powi(3);
        let lerp = |a: f32, b: f32| a + (b - a) * eased;
        // The zoom by how many times larger, the pan with it, so that the
        // way there is even to the eye.
        let zoom = (glide.from.0.ln() + (glide.to.0.ln() - glide.from.0.ln()) * eased).exp();
        let pan = (lerp(glide.from.1 .0, glide.to.1 .0), lerp(glide.from.1 .1, glide.to.1 .1));
        (self.zoom, self.pan) = if t >= 1.0 { glide.to } else { (zoom, pan) };
        self.board.glide = (t < 1.0).then_some(Glide { last: (self.zoom, self.pan), ..glide });
        self.moved_quickly();
    }

    /// How soon the glide wants its next step.
    pub(in crate::studio) fn glide_wait(&self) -> Option<Duration> {
        self.board.glide.map(|_| Duration::from_millis(12))
    }

    /// The glide done at once, where it was going.
    #[cfg(test)]
    pub(in crate::studio) fn end_glide(&mut self) {
        if let Some(glide) = self.board.glide.take() {
            (self.zoom, self.pan) = glide.to;
        }
    }

    /// Part of the board, as a frame of it, brought into view where it is
    /// not wholly in it already: moved only as far as it has to be.
    pub(in crate::studio) fn bring_into_view(&mut self, frame: (f32, f32, f32, f32)) {
        self.view_onto(frame, false);
    }

    /// The same, with room round it, clear of the find bar: moved only as
    /// far as it has to be, or, with `middle`, to the middle; zoomed out to
    /// fit, where it is too large to. In view already, it is left as it is.
    pub(in crate::studio) fn view_onto(&mut self, (x, y, w, h): (f32, f32, f32, f32), middle: bool) {
        let open = self.open_area();
        let top = open.y as f32 + if self.board.find.is_some() { ROOM } else { 0.0 };
        let (left, right, bottom) = (open.x as f32 + ROOM / 2.0, open.right() as f32 - ROOM / 2.0, open.bottom() as f32 - ROOM / 3.0);
        let top = top + ROOM / 3.0;
        let (zoom, pan) = self.view_to();
        let (x, y) = (x + self.doc.x, y + self.doc.y);
        let fits = |zoom: f32| w * zoom <= right - left && h * zoom <= bottom - top;
        let zoom = if fits(zoom) { zoom } else { ((right - left) / w.max(1.0)).min((bottom - top) / h.max(1.0)).clamp(MIN_ZOOM, MAX_ZOOM) };
        let (a, b) = ((x * zoom + pan.0, y * zoom + pan.1), ((x + w) * zoom + pan.0, (y + h) * zoom + pan.1));
        let inside = a.0 >= left && a.1 >= top && b.0 <= right && b.1 <= bottom;
        if inside {
            return;
        }
        let pan = match middle {
            true => ((left + right) / 2.0 - (x + w / 2.0) * zoom, (top + bottom) / 2.0 - (y + h / 2.0) * zoom),
            false => {
                let shift = |low: f32, high: f32, from: f32, to: f32| if low < from { from - low } else if high > to { to - high } else { 0.0 };
                (pan.0 + shift(a.0, b.0, left, right), pan.1 + shift(a.1, b.1, top, bottom))
            }
        };
        self.glide_to(zoom, pan);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::studio::board::tests::*;

    #[test]
    fn the_view_glides_to_where_it_is_going_and_stops_where_it_is_moved_by_hand() {
        let mut studio = board();
        studio.glide_to(1.0, (-400.0, 0.0));
        assert_eq!(studio.pan, (0.0, 0.0), "not jumped");
        assert!(studio.board_wait().is_some_and(|wait| wait <= Duration::from_millis(16)));
        studio.board.glide.as_mut().unwrap().since -= GLIDE / 2;
        studio.board_tick();
        assert!(studio.pan.0 < -200.0 && studio.pan.0 > -400.0, "on the way, eased: {:?}", studio.pan);
        studio.board.glide.as_mut().unwrap().since -= GLIDE;
        studio.board_tick();
        assert_eq!((studio.pan, studio.board.glide.is_none()), ((-400.0, 0.0), true), "there");
        // Moved by hand on the way, it stops.
        studio.glide_to(1.0, (0.0, 0.0));
        studio.board_tick();
        studio.pan.1 += 30.0;
        studio.board_tick();
        assert!(studio.board.glide.is_none());
    }

    #[test]
    fn what_is_brought_into_view_is_moved_only_as_far_as_it_has_to_be() {
        let mut studio = board();
        // In view already: left as it is.
        studio.bring_into_view((300.0, 300.0, 100.0, 60.0));
        assert!(studio.board.glide.is_none());
        // Off the right: moved left until it is in, with room.
        studio.bring_into_view((1500.0, 300.0, 100.0, 60.0));
        studio.end_glide();
        let right = studio.to_window((1600.0, 360.0));
        assert!(right.0 < studio.width - 30 && right.0 > studio.width - 120 && studio.pan.1 == 0.0, "{right:?}");
        // To the middle, and too large: zoomed out to fit.
        studio.view_onto((0.0, 0.0, 4000.0, 400.0), true);
        studio.end_glide();
        assert!(studio.zoom < 0.4);
        let middle = studio.to_window((2000.0, 200.0));
        assert!((middle.0 - studio.open_area().center().0).abs() <= 2);
    }
}

//! Where frames and the page's own layers are among each other, as Figma
//! stacks the things on a page: one order, bottom to top, that they are
//! drawn in and hit in, put in another by the list of layers and by Bring
//! forward and Send backward on a frame or one of the page's own layers.
//! The layers keep their order among themselves, and the frames theirs;
//! each frame says how many of the page's own layers are under it.

use super::*;

impl Studio {
    /// A frame of the page shown, to change: the one worked in, or another.
    pub(super) fn frame_mut(&mut self, index: usize) -> &mut Doc {
        match index == self.active {
            true => &mut self.doc,
            false if index == LOOSE => &mut self.pages[self.page].loose,
            false => &mut self.pages[self.page].frames[index],
        }
    }

    /// Everything on the page shown, bottom to top.
    pub(super) fn page_order(&self) -> Vec<Child> {
        let count = self.loose().layers.len();
        let mut frames: Vec<usize> = (0..self.frame_count()).collect();
        frames.sort_by_key(|&index| (self.frame(index).below.min(count), self.frame(index).tier, index));
        let mut order = Vec::with_capacity(count + frames.len());
        let mut next = 0;
        for index in frames {
            let below = self.frame(index).below.min(count);
            while next < below {
                order.push(Child::Layer(next));
                next += 1;
            }
            order.push(Child::Frame(index));
        }
        order.extend((next..count).map(Child::Layer));
        order
    }

    /// The page's things put in another order, bottom to top: the page's
    /// own layers in theirs, what is picked out of them still picked out,
    /// and each frame over as many of them as come before it.
    pub(super) fn set_page_order(&mut self, order: &[Child]) {
        let layers: Vec<usize> = order.iter().filter_map(|child| match child {
            Child::Layer(index) => Some(*index),
            Child::Frame(_) => None,
        }).collect();
        if layers.iter().enumerate().any(|(at, &index)| at != index) {
            let loose = self.frame_mut(LOOSE);
            let was = std::mem::take(&mut loose.layers);
            loose.layers = layers.iter().map(|&index| was[index].clone()).collect();
            if self.on_page() {
                self.selected = self.selected.iter().filter_map(|&index| layers.iter().position(|&at| at == index)).collect();
                (self.hovered, self.editing, self.cropping) = (None, None, None);
            }
            self.hovered_page = None;
        }
        let (mut under, mut tier) = (0, 0);
        for child in order {
            match *child {
                Child::Layer(_) => under += 1,
                Child::Frame(index) => {
                    let frame = self.frame_mut(index);
                    (frame.below, frame.tier) = (under, tier);
                    tier += 1;
                }
            }
        }
        self.pages[self.page].seen = self.loose().layers.iter().map(stacks::layer_id).collect();
        self.shown_page.clear();
    }

    /// The frames kept where they were among the page's own layers as
    /// those come and go: a frame stays over what it was over. A layer new
    /// at the top of them goes on top of everything, as one drawn on the
    /// page is in Figma; one new among them goes just over the one before
    /// it. The layers only put in another order leave the frames over as
    /// many as they were.
    pub(super) fn keep_order(&mut self) {
        let ids: Vec<u32> = self.loose().layers.iter().map(stacks::layer_id).collect();
        if self.pages[self.page].seen == ids {
            return;
        }
        let seen = std::mem::take(&mut self.pages[self.page].seen);
        let mut sorted_seen = seen.clone();
        let mut sorted_ids = ids.clone();
        sorted_seen.sort_unstable();
        sorted_ids.sort_unstable();
        if sorted_seen != sorted_ids {
            // Each layer's place among the old, a frame's between two: a
            // layer once at p at 2p + 1, a frame over b of them at 2b.
            let last_old = ids.iter().rposition(|id| seen.contains(id));
            let mut ranks = Vec::with_capacity(ids.len());
            let mut before: i64 = -1;
            for (at, id) in ids.iter().enumerate() {
                let rank = match seen.iter().position(|old| old == id) {
                    Some(place) => 2 * place as i64 + 1,
                    None if last_old.is_none_or(|last| at > last) => i64::MAX,
                    None => before,
                };
                ranks.push(rank);
                before = rank;
            }
            for index in 0..self.frame_count() {
                let place = 2 * self.frame(index).below.min(seen.len()) as i64;
                let below = ranks.iter().filter(|&&rank| rank < place).count();
                self.frame_mut(index).below = below;
            }
        }
        let count = ids.len();
        for index in 0..self.frame_count() {
            let frame = self.frame_mut(index);
            frame.below = frame.below.min(count);
        }
        self.pages[self.page].seen = ids;
    }

    /// What on the page a point of it is on, the topmost: a frame, by its
    /// box, or one of the page's own layers that is shown and not locked.
    pub(super) fn top_at(&mut self, (x, y): (f32, f32)) -> Option<Child> {
        let order = self.page_order();
        let layers = self.loose().layers.clone();
        order.into_iter().rev().find(|child| match *child {
            Child::Frame(index) => {
                let frame = self.frame(index);
                x >= frame.x && y >= frame.y && x < frame.x + frame.width as f32 && y < frame.y + frame.height as f32
            }
            Child::Layer(index) => !layers[index].hidden && !layers[index].locked && layers[index].hit((x, y), &mut self.text),
        })
    }

    /// The page's things that are picked out, to put over or under the
    /// rest: the frame picked out, or the page's own layers picked out.
    fn page_picked(&self) -> Vec<Child> {
        if self.frame_picked && !self.on_page() && self.selected.is_empty() {
            return vec![Child::Frame(self.active)];
        }
        match self.on_page() {
            true => self.chosen().into_iter().map(Child::Layer).collect(),
            false => Vec::new(),
        }
    }

    /// Bring forward or send backward, among the things on the page, a
    /// frame picked out or the page's own layers: a step past the next
    /// thing, or with `all` past everything; one step to undo. Whether
    /// there was anything of the page's to do it to.
    pub(super) fn restack_page(&mut self, forward: bool, all: bool) -> bool {
        let picked = self.page_picked();
        if picked.is_empty() {
            return false;
        }
        self.checkpoint();
        let mut order = self.page_order();
        let count = order.len();
        let is = |child: &Child| picked.contains(child);
        if all {
            order.sort_by_key(|child| is(child) == forward);
        } else if forward {
            for at in (0..count.saturating_sub(1)).rev() {
                if is(&order[at]) && !is(&order[at + 1]) {
                    order.swap(at, at + 1);
                }
            }
        } else {
            for at in 1..count {
                if is(&order[at]) && !is(&order[at - 1]) {
                    order.swap(at, at - 1);
                }
            }
        }
        self.set_page_order(&order);
        self.changed();
        true
    }

    /// One thing on the page moved in the order to just over another, or
    /// just under it: where a row dragged in the list is let go.
    pub(super) fn move_child(&mut self, child: Child, by: Child, over: bool) {
        if child == by {
            return;
        }
        let mut order = self.page_order();
        let Some(from) = order.iter().position(|&other| other == child) else { return };
        order.remove(from);
        let Some(at) = order.iter().position(|&other| other == by) else { return };
        order.insert(if over { at + 1 } else { at }, child);
        let (picked, frame) = (self.on_page().then(|| self.chosen()), self.frame_picked);
        self.set_page_order(&order);
        // The one moved picked out where it now is.
        if let (Child::Layer(_), Some(_)) = (child, picked) {
            let index = order.iter().filter(|other| matches!(other, Child::Layer(_))).position(|&other| other == child);
            self.selected = index.into_iter().collect();
        }
        self.frame_picked = frame;
        self.changed();
    }

    /// A frame just added to the page put over everything on it, as a new
    /// frame is in Figma; or a copy of one put just over it.
    pub(super) fn place_new_frame(&mut self, index: usize, over: Option<usize>) {
        let mut order = self.page_order();
        order.retain(|&child| child != Child::Frame(index));
        let at = over.and_then(|source| order.iter().position(|&child| child == Child::Frame(source))).map_or(order.len(), |at| at + 1);
        order.insert(at, Child::Frame(index));
        self.set_page_order(&order);
    }
}

/// What the page is drawn in, bottom to top: a frame; a band of the
/// page's own layers between two frames, from one up to before another,
/// by which band it is; or, where the topmost of the page's own
/// spotlights is, all drawn so far darkened but what they are on.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Piece {
    Frame(usize),
    Band(usize, (usize, usize)),
    Spot,
}

impl Studio {
    /// The page as it is drawn, piece by piece, bottom to top: a spotlight
    /// of the page's own ends a band, as it darkens what is under it of
    /// everything on the page and not only of the page's own layers.
    pub(super) fn pieces(&self) -> Vec<Piece> {
        let layers = &self.loose().layers;
        let spot = |index: usize| matches!(&layers[index].kind, Kind::Shape(shape) if shape.tool == Tool::Spotlight) && !layers[index].hidden;
        let top_spot = (0..layers.len()).rev().find(|&index| spot(index));
        let mut pieces = Vec::new();
        let mut band: Option<(usize, usize)> = None;
        let mut bands = 0;
        let mut close = |band: &mut Option<(usize, usize)>, pieces: &mut Vec<Piece>| {
            if let Some(range) = band.take() {
                pieces.push(Piece::Band(bands, range));
                bands += 1;
            }
        };
        for child in self.page_order() {
            match child {
                Child::Frame(index) => {
                    close(&mut band, &mut pieces);
                    pieces.push(Piece::Frame(index));
                }
                Child::Layer(index) if spot(index) => {
                    close(&mut band, &mut pieces);
                    if top_spot == Some(index) {
                        pieces.push(Piece::Spot);
                    }
                }
                Child::Layer(index) => match &mut band {
                    Some((_, to)) => *to = index + 1,
                    None => band = Some((index, index + 1)),
                },
            }
        }
        close(&mut band, &mut pieces);
        pieces
    }
}

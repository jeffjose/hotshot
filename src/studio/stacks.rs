//! Auto layout, light, as Figma's is cut to what screenshots need: layers
//! in a stack, a row, a column or rows that wrap, an even gap between
//! them, padding round them, lined up one way, in a box that hugs them, is
//! as large as it is set, or fills the frame. Each keeps its own size, and
//! where each goes is worked out: add, remove, resize or drag one along
//! and the rest move up. A stack is itself a container of layers, by what
//! each is called, which the layers do not know of; and can hold another.

use super::*;

/// Which way a stack goes: across, down, or across in rows that wrap.
pub(super) const ROW: u8 = 0;
pub(super) const COLUMN: u8 = 1;
pub(super) const WRAP: u8 = 2;
/// How large its box is: hugging what is in it, as large as it was set,
/// or the whole frame.
pub(super) const HUG: u8 = 0;
pub(super) const FIXED: u8 = 1;
pub(super) const FILL: u8 = 2;

/// Figma's pink for the gaps and padding of a stack, shown while it is
/// hovered.
const PINK: u32 = 0xE5488C;

/// A stack of layers: what it is called, what is in it in its order, by
/// what each is called (a layer, or another stack), which way it goes,
/// the gap between them and the padding round them, how they are lined up
/// (one of nine, left to right and top to bottom), how large its box is,
/// and where that box is on the frame; and what it is filled with.
#[derive(Clone, PartialEq, Debug)]
pub struct Stack {
    pub id: u32,
    pub members: Vec<u32>,
    pub way: u8,
    pub gap: f32,
    pub pad: f32,
    pub align: u8,
    pub sizing: u8,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub fill: Option<u32>,
    pub radius: f32,
    /// What it held, and how large each was, when it was last laid out:
    /// what says that all of it was moved, rather than some of it. Not
    /// kept in its file.
    pub seen: Vec<(u32, f32, f32)>,
    /// What it has been renamed in the list of layers, in place of what it
    /// is called for which way it goes; kept in its file on a line of its
    /// own after it.
    pub name: Option<String>,
}

impl Stack {
    /// As a line of a file.
    pub(super) fn write(&self) -> String {
        let fill = self.fill.map_or("none".to_owned(), |color| format!("{color:06X}"));
        let members: Vec<String> = self.members.iter().map(u32::to_string).collect();
        format!("stack {} {} {} {} {} {} {} {} {} {} {fill} {} {}", self.id, self.way, self.gap, self.pad, self.align, self.sizing, self.x, self.y, self.w, self.h, self.radius, members.join(" "))
    }

    /// A line `write` wrote, after its first word.
    pub(super) fn read(rest: &str) -> Option<Stack> {
        let words: Vec<&str> = rest.split_whitespace().collect();
        let number = |at: usize| words.get(at).and_then(|word| word.parse::<f32>().ok()).filter(|value| value.is_finite());
        let id = words.first()?.parse::<u32>().ok()?;
        shapes::seen_id(id);
        let fill = match *words.get(10)? {
            "none" => None,
            hex => Some(u32::from_str_radix(hex, 16).ok()?),
        };
        let members = words[12.min(words.len())..].iter().filter_map(|word| word.parse::<u32>().ok()).collect();
        Some(Stack {
            id,
            way: (number(1)? as u8).min(WRAP),
            gap: number(2)?,
            pad: number(3)?.max(0.0),
            align: (number(4)? as u8).min(8),
            sizing: (number(5)? as u8).min(FILL),
            x: number(6)?,
            y: number(7)?,
            w: number(8)?.max(1.0),
            h: number(9)?.max(1.0),
            fill,
            radius: number(11)?.max(0.0),
            members,
            seen: Vec::new(),
            name: None,
        })
    }

    /// What the list of layers calls it: what it was renamed, or which way
    /// it goes.
    pub(super) fn title(&self) -> String {
        match &self.name {
            Some(name) => name.clone(),
            None => ["Row", "Column", "Stack"][usize::from(self.way.min(WRAP))].to_owned(),
        }
    }

    /// Lined up how, in words: across, then down.
    pub(super) fn align_says(&self) -> String {
        let across = ["left", "center", "right"][usize::from(self.align % 3)];
        let down = ["top", "middle", "bottom"][usize::from(self.align / 3)];
        format!("{across}, {down}")
    }
}

/// Where each of some boxes goes in a stack, from their sizes, in a frame
/// so large: the stack's box, and the top left of each.
pub(super) fn arrange(stack: &Stack, sizes: &[(f32, f32)], frame: (f32, f32)) -> ((f32, f32, f32, f32), Vec<(f32, f32)>) {
    let across = stack.way != COLUMN;
    // A size as how long it is along the stack, and how deep across it.
    let main = |size: (f32, f32)| if across { size } else { (size.1, size.0) };
    let along = main;
    let (main_align, cross_align) = if across { (stack.align % 3, stack.align / 3) } else { (stack.align / 3, stack.align % 3) };
    let share = |align: u8| f32::from(align) * 0.5;
    let (pad, gap) = (stack.pad.max(0.0), stack.gap);
    let set = match stack.sizing {
        FILL => Some((0.0, 0.0, frame.0, frame.1)),
        FIXED => Some((stack.x, stack.y, stack.w, stack.h)),
        _ => None,
    };
    let set_along = set.map(|(_, _, w, h)| along((w, h)));
    // How far a row can go before it wraps: the width it is set to, or
    // had, less its padding.
    let room = match (set_along, stack.way) {
        (Some((long, _)), _) => Some(long - 2.0 * pad),
        (None, WRAP) => Some(stack.w - 2.0 * pad),
        _ => None,
    };
    let mut lines: Vec<Vec<usize>> = vec![Vec::new()];
    let mut used = 0.0;
    for (at, &size) in sizes.iter().enumerate() {
        let long = main(size).0;
        let line = lines.last_mut().expect("there is always a line");
        if stack.way == WRAP && !line.is_empty() && used + gap + long > room.unwrap_or(f32::MAX) + 0.01 {
            lines.push(vec![at]);
            used = long;
        } else {
            used += if line.is_empty() { long } else { gap + long };
            lines.last_mut().expect("there is always a line").push(at);
        }
    }
    let line_long = |line: &[usize]| line.iter().map(|&at| main(sizes[at]).0).sum::<f32>() + gap * (line.len().max(1) - 1) as f32;
    let line_deep = |line: &[usize]| line.iter().map(|&at| main(sizes[at]).1).fold(0.0f32, f32::max);
    let content_long = lines.iter().map(|line| line_long(line)).fold(0.0f32, f32::max);
    let content_deep = lines.iter().map(|line| line_deep(line)).sum::<f32>() + gap * (lines.len() - 1) as f32;
    let (inner_long, inner_deep) = match set_along {
        Some((long, deep)) => (long - 2.0 * pad, deep - 2.0 * pad),
        None => (room.unwrap_or(content_long), content_deep),
    };
    let size = if across { (inner_long + 2.0 * pad, inner_deep + 2.0 * pad) } else { (inner_deep + 2.0 * pad, inner_long + 2.0 * pad) };
    let (x, y, w, h) = match set {
        Some(set) => set,
        None => (stack.x, stack.y, size.0, size.1),
    };
    let mut tops = vec![(0.0, 0.0); sizes.len()];
    let mut deep_at = pad + (inner_deep - content_deep) * share(cross_align);
    for line in &lines {
        let deep = line_deep(line);
        let mut long_at = pad + (inner_long - line_long(line)) * share(main_align);
        for &at in line {
            let (long, item_deep) = main(sizes[at]);
            let off = (deep - item_deep) * share(cross_align);
            tops[at] = if across { (x + long_at, y + deep_at + off) } else { (x + deep_at + off, y + long_at) };
            long_at += long + gap;
        }
        deep_at += deep + gap;
    }
    ((x, y, w, h), tops)
}

impl Studio {
    /// The stack called this, by where it is.
    pub(super) fn stack_index(&self, id: u32) -> Option<usize> {
        self.doc.stacks.iter().position(|stack| stack.id == id)
    }

    /// The stack something is in, if it is in one, by where that is.
    pub(super) fn stack_of(&self, id: u32) -> Option<usize> {
        self.doc.stacks.iter().position(|stack| stack.members.contains(&id))
    }

    /// The layers in something, by what each is called: a layer itself,
    /// or everything in a stack and the stacks in it.
    pub(super) fn leaves(&self, id: u32) -> Vec<u32> {
        match self.stack_index(id) {
            Some(index) => self.doc.stacks[index].members.iter().flat_map(|&member| self.leaves(member)).collect(),
            None => vec![id],
        }
    }

    /// The box of something on the frame: a layer's, or a stack's.
    pub(super) fn unit_bounds(&mut self, id: u32) -> Option<(f32, f32, f32, f32)> {
        if let Some(index) = self.stack_index(id) {
            let stack = &self.doc.stacks[index];
            return Some((stack.x, stack.y, stack.w, stack.h));
        }
        let index = self.layer_by_id(id)?;
        Some(self.doc.layers[index].bounds(&mut self.text))
    }

    /// Something moved: a layer, or a stack with everything in it.
    pub(super) fn move_unit(&mut self, id: u32, (dx, dy): (f32, f32)) {
        if let Some(index) = self.stack_index(id) {
            let stack = &mut self.doc.stacks[index];
            (stack.x, stack.y) = (stack.x + dx, stack.y + dy);
            for member in stack.members.clone() {
                self.move_unit(member, (dx, dy));
            }
        } else if let Some(index) = self.layer_by_id(id) {
            self.doc.layers[index].translate(dx, dy);
        }
    }

    /// What each layer picked out is called.
    fn chosen_ids(&self) -> Vec<u32> {
        self.chosen().iter().map(|&index| layer_id(&self.doc.layers[index])).collect()
    }

    /// The stack that is what is picked out: the innermost one everything
    /// in which, and nothing else, is picked out.
    pub(super) fn picked_stack(&self) -> Option<usize> {
        let mut chosen = self.chosen_ids();
        if chosen.is_empty() {
            return None;
        }
        // What is in one picked out as each of it, from its row's menu.
        if self.members_picked.as_deref() == Some(&self.selected[..]) {
            return None;
        }
        chosen.sort_unstable();
        let mut best: Option<(usize, usize)> = None;
        for (index, stack) in self.doc.stacks.iter().enumerate() {
            let mut leaves = self.leaves(stack.id);
            leaves.sort_unstable();
            if leaves == chosen {
                let depth = self.depth(stack.id);
                if best.is_none_or(|(_, deeper)| depth > deeper) {
                    best = Some((index, depth));
                }
            }
        }
        best.map(|(index, _)| index)
    }

    /// How many stacks something is inside.
    fn depth(&self, id: u32) -> usize {
        match self.stack_of(id) {
            Some(index) => 1 + self.depth(self.doc.stacks[index].id),
            None => 0,
        }
    }

    /// The innermost stack whose box a point of the frame is in.
    pub(super) fn stack_at(&self, (x, y): (f32, f32)) -> Option<usize> {
        let inside = |stack: &Stack| x >= stack.x && x <= stack.x + stack.w && y >= stack.y && y <= stack.y + stack.h;
        self.doc.stacks.iter().enumerate().filter(|(_, stack)| inside(stack)).max_by_key(|(_, stack)| self.depth(stack.id)).map(|(index, _)| index)
    }

    /// Shift+A: what is picked out made a stack, a row or a column as it
    /// lies, with the gap there is between them now; or the stack picked
    /// out put in one of its own, with what else is picked out.
    pub(super) fn add_stack(&mut self) {
        let chosen = self.chosen_ids();
        if chosen.is_empty() {
            return self.say("Pick some layers to stack them");
        }
        // Whole stacks picked out go in as themselves.
        let mut units: Vec<u32> = Vec::new();
        for &id in &chosen {
            let mut unit = id;
            while let Some(index) = self.stack_of(unit) {
                let outer = self.doc.stacks[index].id;
                if self.leaves(outer).iter().all(|leaf| chosen.contains(leaf)) {
                    unit = outer;
                } else {
                    break;
                }
            }
            if !units.contains(&unit) {
                units.push(unit);
            }
        }
        let mut boxes = Vec::new();
        for &unit in &units {
            boxes.push(self.unit_bounds(unit).unwrap_or_default());
        }
        let row = arrange::in_a_row(&boxes);
        // The gap between them as they are, the commonest, or 16.
        let mut order: Vec<usize> = (0..units.len()).collect();
        order.sort_by(|&a, &b| if row { boxes[a].0.total_cmp(&boxes[b].0) } else { boxes[a].1.total_cmp(&boxes[b].1) });
        let gaps: Vec<f32> = order.windows(2).map(|pair| if row { boxes[pair[1]].0 - (boxes[pair[0]].0 + boxes[pair[0]].2) } else { boxes[pair[1]].1 - (boxes[pair[0]].1 + boxes[pair[0]].3) }).filter(|gap| *gap >= 0.0).collect();
        let gap = if gaps.is_empty() { 16.0 } else { (gaps.iter().sum::<f32>() / gaps.len() as f32).round() };
        let pad = 16.0;
        let (left, top) = (boxes.iter().map(|b| b.0).fold(f32::MAX, f32::min), boxes.iter().map(|b| b.1).fold(f32::MAX, f32::min));
        let (right, bottom) = (boxes.iter().map(|b| b.0 + b.2).fold(f32::MIN, f32::max), boxes.iter().map(|b| b.1 + b.3).fold(f32::MIN, f32::max));
        let members: Vec<u32> = order.iter().map(|&at| units[at]).collect();
        self.checkpoint();
        // What it goes in, if it was all in one: there, in the place of
        // the first of them.
        let outer = members.iter().find_map(|&unit| self.stack_of(unit));
        let id = shapes::fresh_id();
        if let Some(outer) = outer {
            let stack = &mut self.doc.stacks[outer];
            let at = stack.members.iter().position(|member| members.contains(member)).unwrap_or(stack.members.len());
            stack.members.retain(|member| !members.contains(member));
            stack.members.insert(at.min(stack.members.len()), id);
        }
        for stack in &mut self.doc.stacks {
            stack.members.retain(|member| !members.contains(member) || stack.id == id);
        }
        let stack = Stack { id, members, way: if row { ROW } else { COLUMN }, gap, pad, align: 3, sizing: HUG, x: left - pad, y: top - pad, w: right - left + 2.0 * pad, h: bottom - top + 2.0 * pad, fill: None, radius: 0.0, seen: Vec::new(), name: None };
        self.doc.stacks.push(stack);
        self.changed();
        self.say(format!("Auto layout: a {} of {}", if row { "row" } else { "column" }, units.len()));
    }

    /// The stack picked out undone: what was in it stays where it is.
    pub(super) fn remove_stack(&mut self) {
        let Some(index) = self.picked_stack() else { return };
        self.checkpoint();
        let stack = self.doc.stacks.remove(index);
        // What it was in has what it held, in its place.
        if let Some(outer) = self.stack_of(stack.id) {
            let at = self.doc.stacks[outer].members.iter().position(|&member| member == stack.id).unwrap_or(0);
            self.doc.stacks[outer].members.splice(at..=at, stack.members.iter().copied());
        }
        self.changed();
        self.say("Auto layout removed");
    }

    /// A setting of the stack picked out changed: one step to undo.
    pub(super) fn change_stack(&mut self, change: impl Fn(&mut Stack, (f32, f32))) {
        let Some(index) = self.picked_stack() else { return };
        self.checkpoint();
        let frame = (self.doc.width as f32, self.doc.height as f32);
        change(&mut self.doc.stacks[index], frame);
        self.changed();
    }

    /// Stacks that hold nothing gone, and what they held that is gone
    /// gone from them.
    fn prune_stacks(&mut self) {
        let layers: Vec<u32> = self.doc.layers.iter().map(layer_id).collect();
        loop {
            let ids: Vec<u32> = self.doc.stacks.iter().map(|stack| stack.id).collect();
            for stack in &mut self.doc.stacks {
                let own = stack.id;
                stack.members.retain(|member| *member != own && (layers.contains(member) || ids.contains(member)));
                stack.members.dedup();
            }
            let before = self.doc.stacks.len();
            self.doc.stacks.retain(|stack| !stack.members.is_empty());
            if self.doc.stacks.len() == before {
                break;
            }
        }
    }

    /// The stacks laid out again, the innermost first: each moved as a
    /// whole if all of it was moved; or put in order by where each thing
    /// in it now is, and each put where its order says. What is being
    /// dragged stays under the pointer, the rest making room for it.
    /// Whether anything moved.
    pub(super) fn reflow(&mut self) -> bool {
        if self.doc.stacks.is_empty() {
            return false;
        }
        self.prune_stacks();
        let dragged: Vec<u32> = match self.drag {
            Drag::Move { moved: true, .. } => self.chosen_ids(),
            _ => Vec::new(),
        };
        let mut order: Vec<u32> = self.doc.stacks.iter().map(|stack| stack.id).collect();
        order.sort_by_key(|&id| std::cmp::Reverse(self.depth(id)));
        let mut moved = false;
        for id in order {
            moved |= self.reflow_one(id, &dragged);
        }
        self.renumber();
        moved
    }

    fn reflow_one(&mut self, id: u32, dragged: &[u32]) -> bool {
        let Some(index) = self.stack_index(id) else { return false };
        let mut stack = self.doc.stacks[index].clone();
        let mut units = Vec::new();
        for &member in &stack.members {
            if let Some(bounds) = self.unit_bounds(member) {
                units.push((member, bounds));
            }
        }
        let held: Vec<u32> = units
            .iter()
            .map(|&(unit, _)| unit)
            .filter(|&unit| {
                let leaves = self.leaves(unit);
                !leaves.is_empty() && leaves.iter().all(|leaf| dragged.contains(leaf))
            })
            .collect();
        let holding: Vec<bool> = units.iter().map(|(unit, _)| held.contains(unit)).collect();
        let frame = (self.doc.width as f32, self.doc.height as f32);
        // All of it held: its box goes with it as it is dragged, and it is
        // laid out once it is let go of.
        if !holding.is_empty() && holding.iter().all(|&held| held) {
            let sizes: Vec<(f32, f32)> = units.iter().map(|(_, b)| (b.2, b.3)).collect();
            let (_, tops) = arrange(&stack, &sizes, frame);
            let by = (units[0].1 .0 - tops[0].0, units[0].1 .1 - tops[0].1);
            if stack.sizing == FILL || (by.0.abs() < 0.001 && by.1.abs() < 0.001) {
                return false;
            }
            (stack.x, stack.y) = (stack.x + by.0, stack.y + by.1);
            if let Some(index) = self.stack_index(id) {
                self.doc.stacks[index] = stack;
            }
            return true;
        }
        let sizes: Vec<(f32, f32)> = units.iter().map(|(_, b)| (b.2, b.3)).collect();
        let seen: Vec<(u32, f32, f32)> = units.iter().map(|&(unit, b)| (unit, b.2, b.3)).collect();
        let (_, tops) = arrange(&stack, &sizes, frame);
        let deltas: Vec<(f32, f32)> = units.iter().zip(&tops).zip(&holding).filter(|(_, held)| !**held).map(|((&(_, b), top), _)| (b.0 - top.0, b.1 - top.1)).collect();
        let same = |a: (f32, f32), b: (f32, f32)| (a.0 - b.0).abs() < 0.05 && (a.1 - b.1).abs() < 0.05;
        let still = deltas.iter().all(|&delta| same(delta, (0.0, 0.0)));
        let together = deltas.windows(2).all(|pair| same(pair[0], pair[1]));
        if !holding.contains(&true) && stack.seen == seen && together && !still && stack.sizing != FILL {
            // All of it was moved: its box goes with it.
            let by = deltas[0];
            (stack.x, stack.y) = (stack.x + by.0, stack.y + by.1);
        } else if !still || holding.contains(&true) {
            // In order by where each is now, along the stack; a wrap's by
            // the row each is in, then along it.
            let centers: Vec<(f32, f32)> = units.iter().map(|&(_, b)| (b.0 + b.2 / 2.0, b.1 + b.3 / 2.0)).collect();
            let tallest = units.iter().map(|&(_, b)| b.3).fold(1.0f32, f32::max) + stack.gap.max(0.0);
            let top = units.iter().map(|&(_, b)| b.1).fold(f32::MAX, f32::min);
            let mut order: Vec<usize> = (0..units.len()).collect();
            let key = |at: usize| match stack.way {
                ROW => (0.0, centers[at].0),
                COLUMN => (0.0, centers[at].1),
                _ => (((centers[at].1 - top) / tallest).floor(), centers[at].0),
            };
            order.sort_by(|&a, &b| key(a).partial_cmp(&key(b)).unwrap_or(std::cmp::Ordering::Equal));
            let missing: Vec<u32> = stack.members.iter().copied().filter(|member| !units.iter().any(|(unit, _)| unit == member)).collect();
            stack.members = order.iter().map(|&at| units[at].0).chain(missing).collect();
            units = order.iter().map(|&at| units[at]).collect();
        }
        let sizes: Vec<(f32, f32)> = units.iter().map(|(_, b)| (b.2, b.3)).collect();
        let (bounds, tops) = arrange(&stack, &sizes, frame);
        let mut moved = false;
        for (&(unit, b), top) in units.iter().zip(&tops) {
            if held.contains(&unit) {
                continue;
            }
            let by = (top.0 - b.0, top.1 - b.1);
            if by.0.abs() > 0.001 || by.1.abs() > 0.001 {
                self.move_unit(unit, by);
                moved = true;
            }
        }
        (stack.x, stack.y, stack.w, stack.h) = bounds;
        stack.seen = units.iter().map(|&(unit, b)| (unit, b.2, b.3)).collect();
        if let Some(index) = self.stack_index(id) {
            moved |= self.doc.stacks[index] != stack;
            // The box is where its members now are; the same layers moved
            // into it are what it saw.
            self.doc.stacks[index] = stack;
        }
        moved
    }

    /// The counters in each stack numbered by their order in it, the
    /// stacks in it gone through as they come.
    fn renumber(&mut self) {
        let tops: Vec<u32> = self.doc.stacks.iter().filter(|stack| self.stack_of(stack.id).is_none()).map(|stack| stack.id).collect();
        for top in tops {
            let mut number = 0;
            for leaf in self.leaves(top) {
                if let Some(index) = self.layer_by_id(leaf) {
                    if let Kind::Shape(shape) = &mut self.doc.layers[index].kind {
                        if shape.tool == Tool::Counter {
                            number += 1;
                            if shape.number != number {
                                shape.number = number;
                                shape.touch();
                            }
                        }
                    }
                }
            }
        }
    }

    /// What was dragged let go: out of the stack it was in, if it was let
    /// go well outside it; into the stack it was let go in, if it was in
    /// none. Laid out again, either way, with the rest.
    pub(super) fn drop_into_stacks(&mut self) {
        let chosen = self.chosen_ids();
        if chosen.is_empty() || self.doc.stacks.is_empty() {
            return;
        }
        // What was dragged as a thing of its own: a layer, or a stack all
        // of which was dragged.
        let mut units: Vec<u32> = Vec::new();
        for &id in &chosen {
            let mut unit = id;
            while let Some(index) = self.stack_of(unit) {
                let outer = self.doc.stacks[index].id;
                if self.leaves(outer).iter().all(|leaf| chosen.contains(leaf)) {
                    unit = outer;
                } else {
                    break;
                }
            }
            if !units.contains(&unit) {
                units.push(unit);
            }
        }
        for unit in units {
            let Some((x, y, w, h)) = self.unit_bounds(unit) else { continue };
            let middle = (x + w / 2.0, y + h / 2.0);
            match self.stack_of(unit) {
                Some(index) => {
                    // Dragged out: wholly off its box.
                    let stack = &self.doc.stacks[index];
                    let out = x > stack.x + stack.w || x + w < stack.x || y > stack.y + stack.h || y + h < stack.y;
                    if out {
                        self.doc.stacks[index].members.retain(|&member| member != unit);
                    }
                }
                None => {
                    // Dropped in: into the innermost box it is let go in
                    // that is not its own.
                    let inside: Vec<usize> = (0..self.doc.stacks.len())
                        .filter(|&index| {
                            let stack = &self.doc.stacks[index];
                            stack.id != unit && !self.leaves(unit).contains(&stack.id) && !self.inside_of(stack.id, unit) && middle.0 >= stack.x && middle.0 <= stack.x + stack.w && middle.1 >= stack.y && middle.1 <= stack.y + stack.h
                        })
                        .collect();
                    if let Some(&index) = inside.iter().max_by_key(|&&index| self.depth(self.doc.stacks[index].id)) {
                        self.doc.stacks[index].members.push(unit);
                    }
                }
            }
        }
    }

    /// Whether a stack is inside another, however deep.
    fn inside_of(&self, id: u32, outer: u32) -> bool {
        match self.stack_of(id) {
            Some(index) => self.doc.stacks[index].id == outer || self.inside_of(self.doc.stacks[index].id, outer),
            None => false,
        }
    }

    /// A copy just made of something in a stack goes in it next after
    /// what it is a copy of, where that is, to be laid out there.
    pub(super) fn stack_copies(&mut self, pairs: &[(u32, u32)]) {
        if self.doc.stacks.is_empty() || pairs.is_empty() {
            return;
        }
        let sources: Vec<u32> = pairs.iter().map(|&(source, _)| source).collect();
        let whole = |studio: &Studio, id: u32| studio.leaves(id).iter().all(|leaf| sources.contains(leaf));
        // A whole stack copied is a stack too, beside it as the copies are,
        // and next after it in what it is in.
        let tops: Vec<u32> = self.doc.stacks.iter().map(|stack| stack.id).filter(|&id| whole(self, id) && self.stack_of(id).is_none_or(|outer| !whole(self, self.doc.stacks[outer].id))).collect();
        let mut handled: Vec<u32> = Vec::new();
        for top in tops {
            let leaves = self.leaves(top);
            let Some(&(source, copy)) = pairs.iter().find(|(source, _)| leaves.contains(source)) else { continue };
            let (Some(a), Some(b)) = (self.unit_bounds(source), self.unit_bounds(copy)) else { continue };
            let copied = self.copy_stack(top, pairs, (b.0 - a.0, b.1 - a.1));
            if let Some(outer) = self.stack_of(top) {
                let stack = &mut self.doc.stacks[outer];
                let at = stack.members.iter().position(|&member| member == top).map_or(stack.members.len(), |at| at + 1);
                stack.members.insert(at, copied);
            }
            handled.extend(leaves);
        }
        // The rest: what is copied of a stack goes in it after the last of
        // what it is a copy of, in order, put where that last is, so that
        // it is laid out after it.
        let rest: Vec<(u32, u32)> = pairs.iter().copied().filter(|(source, _)| !handled.contains(source)).collect();
        let mut stacks: Vec<usize> = rest.iter().filter_map(|&(source, _)| self.stack_of(source)).collect();
        stacks.dedup();
        for index in stacks {
            let members = self.doc.stacks[index].members.clone();
            let mine: Vec<(u32, u32)> = rest.iter().copied().filter(|(source, _)| members.contains(source)).collect();
            let Some(last) = members.iter().rposition(|member| mine.iter().any(|(source, _)| source == member)) else { continue };
            let last_id = members[last];
            for (offset, &(_, copy)) in mine.iter().enumerate() {
                self.doc.stacks[index].members.insert(last + 1 + offset, copy);
                if let (Some(from), Some(to)) = (self.layer_by_id(last_id), self.layer_by_id(copy)) {
                    let (a, b) = (self.doc.layers[from].bounds(&mut self.text), self.doc.layers[to].bounds(&mut self.text));
                    self.doc.layers[to].translate(a.0 - b.0, a.1 - b.1);
                }
            }
        }
    }

    /// A stack copied, with the stacks in it, holding the copies made of
    /// what it holds, its box moved as they were: what the copy is called.
    fn copy_stack(&mut self, id: u32, pairs: &[(u32, u32)], (dx, dy): (f32, f32)) -> u32 {
        let Some(index) = self.stack_index(id) else { return id };
        let mut stack = self.doc.stacks[index].clone();
        let mut members = Vec::new();
        for member in stack.members.clone() {
            members.push(match self.stack_index(member) {
                Some(_) => self.copy_stack(member, pairs, (dx, dy)),
                None => pairs.iter().find(|(source, _)| *source == member).map_or(member, |&(_, copy)| copy),
            });
        }
        (stack.id, stack.members, stack.seen) = (shapes::fresh_id(), members, Vec::new());
        (stack.x, stack.y) = (stack.x + dx, stack.y + dy);
        let copied = stack.id;
        self.doc.stacks.push(stack);
        copied
    }

    /// The stack the pointer is over on the frame, with the move tool in
    /// hand, for its gaps and padding to show.
    pub(super) fn stack_hover(&mut self, at: (f32, f32)) -> Option<u32> {
        if self.tool != Tool::Select || self.doc.stacks.is_empty() {
            return None;
        }
        self.stack_at(at).map(|index| self.doc.stacks[index].id)
    }

    /// The box of the stack picked out ringed, and the gaps and padding of
    /// the one hovered, or whose Gap or Pad is hovered, in pink with how
    /// large each is, as Figma shows them.
    pub(super) fn draw_stacks(&mut self, canvas: &mut Canvas, hover: Option<Control>) {
        if self.doc.stacks.is_empty() {
            return;
        }
        let picked = self.picked_stack();
        if let Some(index) = picked {
            let stack = self.doc.stacks[index].clone();
            let (a, b) = (self.to_window((stack.x, stack.y)), self.to_window((stack.x + stack.w, stack.y + stack.h)));
            canvas.round_outline(a.0 - 1, a.1 - 1, b.0 - a.0 + 2, b.1 - a.1 + 2, 0, 1, ACCENT);
        }
        let fields = matches!(hover, Some(Control::Field(Field::StackGap | Field::StackPad)));
        let shown = match (fields, picked) {
            (true, Some(index)) => Some(index),
            _ => self.stack_hovered.and_then(|id| self.stack_index(id)).filter(|_| self.drag == Drag::None),
        };
        let Some(index) = shown else { return };
        let stack = self.doc.stacks[index].clone();
        let mut boxes = Vec::new();
        for &member in &stack.members {
            if let Some(bounds) = self.unit_bounds(member) {
                boxes.push(bounds);
            }
        }
        let to = |studio: &Studio, (x, y): (f32, f32)| studio.to_window((x, y));
        // The padding: the band inside each side, striped.
        let (a, b) = (to(self, (stack.x, stack.y)), to(self, (stack.x + stack.w, stack.y + stack.h)));
        let pad = (stack.pad * self.zoom).round() as i32;
        if pad > 0 {
            let bands = [(a.0, a.1, b.0 - a.0, pad), (a.0, b.1 - pad, b.0 - a.0, pad), (a.0, a.1 + pad, pad, b.1 - a.1 - 2 * pad), (b.0 - pad, a.1 + pad, pad, b.1 - a.1 - 2 * pad)];
            for (x, y, w, h) in bands {
                let band = Rect::new(x, y, w, h).intersect(Rect::new(0, 0, self.width, self.height));
                for py in band.y..band.bottom() {
                    for px in band.x..band.right() {
                        let stripe = (px + py).rem_euclid(6) < 3;
                        canvas.blend(px, py, PINK, if stripe { 72 } else { 24 });
                    }
                }
            }
        }
        // The gaps: between each and the next along the stack, as tall as
        // the taller of the two, with how wide in the middle.
        let across = stack.way != COLUMN;
        let mut labels = Vec::new();
        for pair in boxes.windows(2) {
            let (p, q) = (pair[0], pair[1]);
            let gap = if across {
                // A wrap's next row begins afresh.
                if q.0 < p.0 + p.2 - 0.5 {
                    continue;
                }
                let (top, bottom) = (p.1.min(q.1), (p.1 + p.3).max(q.1 + q.3));
                (p.0 + p.2, top, q.0 - (p.0 + p.2), bottom - top)
            } else {
                let (left, right) = (p.0.min(q.0), (p.0 + p.2).max(q.0 + q.2));
                (left, p.1 + p.3, right - left, q.1 - (p.1 + p.3))
            };
            if gap.2 <= 0.0 || gap.3 <= 0.0 {
                continue;
            }
            let (g0, g1) = (to(self, (gap.0, gap.1)), to(self, (gap.0 + gap.2, gap.1 + gap.3)));
            canvas.round_fill(g0.0, g0.1, (g1.0 - g0.0).max(1), (g1.1 - g0.1).max(1), 0, PINK, 64);
            labels.push(((g0.0 + g1.0) / 2, (g0.1 + g1.1) / 2));
        }
        let says = format!("{}", stack.gap.round() as i32);
        for (cx, cy) in labels {
            let (wide, high) = (self.text.width(&says, 11) + 8, Text::line_height(11) + 2);
            canvas.round_fill(cx - wide / 2, cy - high / 2, wide, high, 4, PINK, 256);
            self.text.draw(canvas, cx - wide / 2 + 4, cy - high / 2 + 1, 11, 0xFFFFFF, &says);
        }
        if pad > 0 {
            let says = format!("{}", stack.pad.round() as i32);
            let (wide, high) = (self.text.width(&says, 11) + 8, Text::line_height(11) + 2);
            let (cx, cy) = ((a.0 + b.0) / 2, a.1 + pad / 2);
            canvas.round_fill(cx - wide / 2, cy - high / 2, wide, high, 4, PINK, 256);
            self.text.draw(canvas, cx - wide / 2 + 4, cy - high / 2 + 1, 11, 0xFFFFFF, &says);
        }
    }

    /// The Auto layout section of the panel, as Figma's: which way, the
    /// gap and the padding, how they are lined up, and how large the box
    /// is; and what it is filled with. For what is picked out that is no
    /// stack, the heading and a plus to make it one.
    pub(super) fn stack_section(&mut self, rows: &mut panel::Rows) {
        let Some(index) = self.picked_stack() else {
            rows.section("Auto layout", "Shift A", Some((Control::Action(Action::AddStack), icons::PLUS)));
            return;
        };
        let stack = self.doc.stacks[index].clone();
        rows.section("Auto layout", "Shift A", Some((Control::Action(Action::RemoveStack), icons::MINUS)));
        let ways = [("\u{2192} Row", ROW), ("\u{2193} Column", COLUMN), ("Wrap", WRAP)].iter().map(|&(says, way)| (says.to_owned(), Control::StackWay(way))).collect();
        rows.labeled(&mut self.text, "", ways);
        rows.fields(&[Field::StackGap, Field::StackPad], &[]);
        rows.align_grid("Align", stack.align_says());
        let sizings = [("Hug", HUG), ("Fixed", FIXED), ("Fill frame", FILL)].iter().map(|&(says, sizing)| (says.to_owned(), Control::StackSizing(sizing))).collect();
        rows.labeled(&mut self.text, "Size", sizings);
        rows.labeled_wide("Fill", Control::ColorField(Paint::Stack));
    }

    /// Whether a chip of the Auto layout section is how the stack picked
    /// out is.
    pub(super) fn stack_chip_on(&self, control: Control) -> bool {
        let Some(stack) = self.picked_stack().map(|index| &self.doc.stacks[index]) else { return false };
        match control {
            Control::StackWay(way) => stack.way == way,
            Control::StackSizing(sizing) => stack.sizing == sizing,
            Control::StackAlign(align) => stack.align == align,
            _ => false,
        }
    }

    /// What a press of a control of the section does.
    pub(super) fn press_stack(&mut self, control: Control) {
        match control {
            Control::StackWay(way) => self.change_stack(|stack, _| {
                // A wrap goes as wide as the stack was.
                stack.way = way;
            }),
            Control::StackSizing(sizing) => self.change_stack(|stack, _| stack.sizing = sizing),
            Control::StackAlign(align) => self.change_stack(|stack, _| stack.align = align.min(8)),
            _ => {}
        }
    }
}

/// What a layer is called, which no other is.
pub(super) fn layer_id(layer: &Layer) -> u32 {
    match &layer.kind {
        Kind::Picture(picture) => picture.id,
        Kind::Shape(shape) => shape.id,
    }
}

/// The stacks' fills drawn into part of a frame drawn at `times` its size:
/// each that is under the layer about to be drawn, by its id.
pub(super) fn draw_fills(stacks: &[Stack], under: u32, firsts: &[(u32, u32)], out: &mut [u32], part: Rect, times: f32) {
    for &(id, first) in firsts {
        let Some(stack) = stacks.iter().find(|stack| stack.id == id).filter(|_| first == under) else { continue };
        let Some(color) = stack.fill else { continue };
        let (x, y, w, h) = (stack.x * times - part.x as f32, stack.y * times - part.y as f32, stack.w * times, stack.h * times);
        let radius = (stack.radius * times).min(w.min(h) / 2.0);
        let area = Rect::new(x.floor() as i32, y.floor() as i32, w.ceil() as i32 + 2, h.ceil() as i32 + 2).intersect(Rect::new(0, 0, part.w, part.h));
        for py in area.y..area.bottom() {
            for px in area.x..area.right() {
                let away = frame::round_box(px as f32 + 0.5 - x - w / 2.0, py as f32 + 0.5 - y - h / 2.0, w / 2.0, h / 2.0, radius);
                let cover = (0.5 - away).clamp(0.0, 1.0);
                if cover > 0.0 {
                    let pixel = &mut out[(py * part.w + px) as usize];
                    *pixel = over(*pixel, color, cover);
                }
            }
        }
    }
}

/// For each stack with a fill, the layer it is drawn under: the first of
/// what is in it, among the layers, as they are ordered; those a stack is
/// in before it, to be drawn under it.
pub(super) fn fill_firsts(doc: &Doc) -> Vec<(u32, u32)> {
    let depth = |id: u32| {
        let (mut depth, mut at) = (0, id);
        while let Some(outer) = doc.stacks.iter().find(|stack| stack.members.contains(&at)) {
            (depth, at) = (depth + 1, outer.id);
            if depth > doc.stacks.len() {
                break;
            }
        }
        depth
    };
    let mut firsts = fill_firsts_unordered(doc);
    firsts.sort_by_key(|&(id, _)| depth(id));
    firsts
}

fn fill_firsts_unordered(doc: &Doc) -> Vec<(u32, u32)> {
    let order: Vec<u32> = doc.layers.iter().map(layer_id).collect();
    let leaves = |id: u32| -> Vec<u32> {
        let mut found = Vec::new();
        let mut todo = vec![id];
        while let Some(next) = todo.pop() {
            match doc.stacks.iter().find(|stack| stack.id == next) {
                Some(stack) => todo.extend(stack.members.iter().copied()),
                None => found.push(next),
            }
        }
        found
    };
    doc.stacks
        .iter()
        .filter(|stack| stack.fill.is_some())
        .filter_map(|stack| {
            let first = leaves(stack.id).into_iter().filter_map(|leaf| order.iter().position(|&id| id == leaf)).min()?;
            Some((stack.id, order[first]))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack(way: u8, sizing: u8, align: u8) -> Stack {
        Stack { id: 1, members: vec![], way, gap: 10.0, pad: 20.0, align, sizing, x: 100.0, y: 50.0, w: 200.0, h: 300.0, fill: None, radius: 0.0, seen: Vec::new(), name: None }
    }

    #[test]
    fn a_stack_lays_out_a_row_a_column_and_a_wrap_each_keeping_its_size() {
        let sizes = [(40.0, 30.0), (60.0, 50.0), (20.0, 10.0)];
        // A row hugging them: the padding, then each after the gap, lined
        // up along their middles.
        let (bounds, tops) = arrange(&stack(ROW, HUG, 3), &sizes, (1000.0, 1000.0));
        assert_eq!(bounds, (100.0, 50.0, 20.0 + 40.0 + 10.0 + 60.0 + 10.0 + 20.0 + 20.0, 20.0 + 50.0 + 20.0));
        assert_eq!(tops, vec![(120.0, 80.0), (170.0, 70.0), (240.0, 90.0)]);
        // A column, lined up at the right.
        let (bounds, tops) = arrange(&stack(COLUMN, HUG, 2), &sizes, (1000.0, 1000.0));
        assert_eq!((bounds.2, bounds.3), (100.0, 20.0 + 30.0 + 10.0 + 50.0 + 10.0 + 10.0 + 20.0));
        assert_eq!(tops, vec![(140.0, 70.0), (120.0, 110.0), (160.0, 170.0)]);
        // Fixed at 200 wide, a wrap goes on to a second row.
        let (bounds, tops) = arrange(&stack(WRAP, FIXED, 0), &[(70.0, 20.0), (70.0, 20.0), (70.0, 20.0)], (1000.0, 1000.0));
        assert_eq!(bounds, (100.0, 50.0, 200.0, 300.0));
        assert_eq!(tops, vec![(120.0, 70.0), (200.0, 70.0), (120.0, 100.0)]);
        // Filling the frame, in its middle.
        let (bounds, tops) = arrange(&stack(ROW, FILL, 4), &[(100.0, 100.0)], (400.0, 300.0));
        assert_eq!((bounds, tops), ((0.0, 0.0, 400.0, 300.0), vec![(150.0, 100.0)]));
        let back = Stack::read(&stack(WRAP, FIXED, 5).write()["stack ".len()..]).unwrap();
        assert_eq!(back, stack(WRAP, FIXED, 5));
    }

    fn studio() -> Studio {
        let mut studio = Studio::new(Doc::new(800, 600), Text::load(), 1400, 900, std::env::temp_dir(), "test-%Y.png".into());
        (studio.zoom, studio.pan) = (1.0, (300.0, 100.0));
        studio
    }

    /// A picture so large, its top left at a point of the frame.
    fn put(studio: &mut Studio, (x, y): (f32, f32), (w, h): (f32, f32)) -> usize {
        let image = Arc::new(Image::new(w as i32, h as i32, vec![0xFF3E63DD; (w * h) as usize]));
        studio.doc.layers.push(Layer::picture(Picture::new(image, "box".into(), (x + w / 2.0, y + h / 2.0))));
        studio.doc.layers.len() - 1
    }

    fn corner(studio: &mut Studio, index: usize) -> (f32, f32) {
        let (x, y, _, _) = studio.doc.layers[index].bounds(&mut studio.text);
        (x.round(), y.round())
    }

    const SHIFT: Mods = Mods { ctrl: false, shift: true, alt: false };
    const CTRL: Mods = Mods { ctrl: true, shift: false, alt: false };

    fn drag(studio: &mut Studio, from: (f32, f32), to: (f32, f32)) {
        let (a, b) = (studio.to_window(from), studio.to_window(to));
        studio.pointer_moved(a.0, a.1, Mods::default());
        studio.last_press.0 = 0;
        studio.pressed(1, 100_000);
        studio.pointer_moved((a.0 + b.0) / 2, (a.1 + b.1) / 2, Mods::default());
        studio.pointer_moved(b.0, b.1, Mods::default());
        studio.released();
    }

    #[test]
    fn shift_a_stacks_what_is_picked_out_and_it_reflows_as_it_changes() {
        let mut studio = studio();
        let a = put(&mut studio, (100.0, 100.0), (40.0, 60.0));
        let b = put(&mut studio, (170.0, 110.0), (40.0, 40.0));
        let c = put(&mut studio, (240.0, 100.0), (40.0, 60.0));
        studio.selected = vec![c, a, b];
        studio.key(Key::Char('A'), SHIFT, true);
        assert_eq!(studio.doc.stacks.len(), 1);
        let stack = studio.doc.stacks[0].clone();
        // A row, as they lay, with the gap they had between them, and
        // lined up along their middles.
        assert_eq!((stack.way, stack.gap, stack.members.len()), (ROW, 30.0, 3));
        assert_eq!(corner(&mut studio, a), (100.0, 100.0));
        assert_eq!(corner(&mut studio, b), (170.0, 110.0));
        assert_eq!(corner(&mut studio, c), (240.0, 100.0));
        assert_eq!(studio.picked_stack(), Some(0));
        // The gap made 10: the rest move up.
        studio.change_stack(|stack, _| stack.gap = 10.0);
        assert_eq!((corner(&mut studio, b), corner(&mut studio, c)), ((150.0, 110.0), (200.0, 100.0)));
        // The first made wider: the rest move along.
        if let Kind::Picture(picture) = &mut studio.doc.layers[a].kind {
            (picture.w, picture.cx) = (picture.w + 20.0, picture.cx + 10.0);
        }
        studio.changed();
        assert_eq!((corner(&mut studio, b), corner(&mut studio, c)), ((170.0, 110.0), (220.0, 100.0)));
        // All of it moved: its box goes with it.
        studio.selected = vec![a, b, c];
        studio.key(Key::Right, SHIFT, true);
        assert_eq!(corner(&mut studio, a), (110.0, 100.0));
        assert_eq!(studio.doc.stacks[0].x, 110.0 - 16.0);
        // The first dragged past the last: it goes at the end, the others
        // move up to where it was.
        studio.selected = vec![a];
        drag(&mut studio, (130.0, 130.0), (250.0, 130.0));
        let ids: Vec<u32> = studio.doc.stacks[0].members.clone();
        assert_eq!(ids.last().copied(), Some(layer_id(&studio.doc.layers[a])));
        assert_eq!(corner(&mut studio, b), (110.0, 110.0));
        assert_eq!(corner(&mut studio, c), (160.0, 100.0));
        assert_eq!(corner(&mut studio, a), (210.0, 100.0));
        // One deleted: the one after it moves up into its place.
        studio.selected = vec![c];
        studio.key(Key::Delete, Mods::default(), true);
        let a = studio.doc.layers.iter().position(|layer| layer_id(layer) == *ids.last().unwrap()).unwrap();
        assert_eq!(corner(&mut studio, a), (160.0, 100.0));
        // Undone, back as it was; and the stack is kept in the file.
        studio.key(Key::Char('z'), CTRL, true);
        assert_eq!(studio.doc.stacks[0].members.len(), 3);
        let read = Doc::read(&studio.doc.write()).unwrap();
        assert_eq!(read.stacks[0].members, studio.doc.stacks[0].members);
    }

    #[test]
    fn counters_in_a_stack_are_numbered_by_its_order_and_ctrl_d_adds_the_next() {
        let mut studio = studio();
        let mut counters = Vec::new();
        for (at, number) in [(100.0, 3), (100.0 + 60.0, 1)] {
            let mut shape = Shape::new(Tool::Counter, 0xE5484D, 2.0, (at, 100.0));
            shape.number = number;
            studio.doc.layers.push(Layer::shape(shape));
            counters.push(studio.doc.layers.len() - 1);
        }
        studio.selected = counters.clone();
        studio.key(Key::Char('A'), SHIFT, true);
        let number = |studio: &Studio, index: usize| match &studio.doc.layers[index].kind {
            Kind::Shape(shape) => shape.number,
            _ => 0,
        };
        assert_eq!((number(&studio, counters[0]), number(&studio, counters[1])), (1, 2));
        // Ctrl+D on the first: the copy goes next after it, numbered 2,
        // and the one after it is 3, pushed along by the stack's gap.
        studio.selected = vec![counters[0]];
        let before = corner(&mut studio, counters[1]);
        studio.key(Key::Char('d'), CTRL, true);
        let copy = studio.doc.layers.len() - 1;
        assert_eq!(studio.doc.stacks[0].members[1], layer_id(&studio.doc.layers[copy]));
        assert_eq!((number(&studio, copy), number(&studio, counters[1])), (2, 3));
        assert!(corner(&mut studio, counters[1]).0 > before.0);
        assert_eq!(corner(&mut studio, copy), before);
        // All of it copied: a stack of its own, beside it, numbered again
        // from 1; the first as it was.
        studio.selected = (0..studio.doc.layers.len()).collect();
        assert_eq!(studio.picked_stack(), Some(0));
        studio.key(Key::Char('d'), CTRL, true);
        assert_eq!((studio.doc.stacks.len(), studio.doc.stacks[0].members.len(), studio.doc.stacks[1].members.len()), (2, 3, 3));
        assert_eq!(studio.picked_stack(), Some(1));
        let numbers: Vec<u32> = studio.doc.stacks[1].members.iter().map(|&id| number(&studio, studio.layer_by_id(id).unwrap())).collect();
        assert_eq!(numbers, [1, 2, 3]);
        assert!(studio.doc.stacks[1].x > studio.doc.stacks[0].x + studio.doc.stacks[0].w - 1.0 || studio.doc.stacks[1].y > studio.doc.stacks[0].y + studio.doc.stacks[0].h - 1.0);
    }

    #[test]
    fn a_stack_is_picked_out_by_its_padding_and_shows_its_gaps_and_a_layer_dropped_in_joins_it() {
        let mut studio = studio();
        let a = put(&mut studio, (100.0, 100.0), (40.0, 40.0));
        let b = put(&mut studio, (160.0, 100.0), (40.0, 40.0));
        let loose = put(&mut studio, (400.0, 400.0), (40.0, 40.0));
        studio.selected = vec![a, b];
        studio.key(Key::Char('A'), SHIFT, true);
        studio.selected.clear();
        // A click in its padding picks all of it out.
        let pad = studio.to_window((100.0 - 8.0, 120.0));
        studio.pointer_moved(pad.0, pad.1, Mods::default());
        studio.last_press.0 = 0;
        studio.pressed(1, 200_000);
        studio.released();
        assert_eq!(studio.picked_stack(), Some(0));
        // The panel has its section, as Figma's.
        let layout = studio.layout();
        assert!(layout.controls.iter().any(|(_, control)| *control == Control::StackWay(COLUMN)));
        assert_eq!(layout.controls.iter().filter(|(_, control)| matches!(control, Control::StackAlign(_))).count(), 9);
        // Made a column from the panel.
        studio.press_stack(Control::StackWay(COLUMN));
        assert_eq!(corner(&mut studio, b), (100.0, 160.0));
        // Filled, its fill is under what is in it, in its padding too.
        studio.paint(Paint::Stack, Some(0xFFFFFF));
        studio.doc.background = Fill::Color(0xE4E4E7);
        let (wide, _, pixels) = studio.doc.render(1.0, &mut studio.text);
        assert_eq!(pixels[(120 * wide + 92) as usize] & 0xFF_FFFF, 0xFFFFFF);
        assert_eq!(pixels[(120 * wide + 120) as usize] & 0xFF_FFFF, 0x3E63DD);
        assert_eq!(pixels[(120 * wide + 60) as usize] & 0xFF_FFFF, 0xE4E4E7, "the frame's own outside it");
        // A loose box dropped in its box joins it, at the end.
        studio.selected = vec![loose];
        drag(&mut studio, (420.0, 420.0), (120.0, 210.0));
        assert_eq!(studio.doc.stacks[0].members.len(), 3);
        assert_eq!(corner(&mut studio, loose), (100.0, 220.0));
        // And dragged well out, it leaves.
        drag(&mut studio, (120.0, 240.0), (500.0, 500.0));
        assert_eq!(studio.doc.stacks[0].members.len(), 2);
        // Removed, what was in it stays where it is.
        studio.selected = vec![a, b];
        studio.remove_stack();
        assert!(studio.doc.stacks.is_empty());
        assert_eq!(corner(&mut studio, b), (100.0, 160.0));
    }
}

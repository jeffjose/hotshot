//! A board the same whichever way it is come at: in each look, what is
//! shown while something is done is what it does; what each part of the
//! panel sets is all it sets; a look changes how a board is drawn, not
//! what is on it; each thing done is one step to undo; and the file keeps
//! all of it.

use super::tests::*;
use super::*;

/// What a shape is, by name, as a board's panel and file have it: what
/// two of the same are compared by, and two differ in.
fn fingerprint(shape: &Shape) -> Vec<(&'static str, String)> {
    let (x, y, w, h) = shape.frame();
    let round = |v: f32| (v * 2.0).round() / 2.0;
    vec![
        ("color", format!("{:06X}", shape.color)),
        ("fill", format!("{:06X}", shape.fill)),
        ("filled", shape.filled.to_string()),
        ("hatch", shape.hatch.to_string()),
        ("dashed", shape.dashed.to_string()),
        ("dotted", shape.dotted.to_string()),
        ("stroke", format!("{:.2}", shape.stroke)),
        ("font_px", shape.font_px.to_string()),
        ("nominal", shape.nominal.to_string()),
        ("face", shape.face.to_string()),
        ("heads", format!("{:?}", shape.heads)),
        ("ends", format!("{:?}", shape.ends)),
        ("routed", shape.routed.to_string()),
        ("elbow", shape.elbow.to_string()),
        ("step", format!("{:?}", shape.step)),
        ("sketch", shape.sketch.to_string()),
        ("form", shape.form.to_string()),
        ("text", shape.text.clone()),
        ("stuck", format!("{:?}", shape.stuck.map(|end| end.map(|(id, ..)| id)))),
        ("frame", format!("{} {} {} {}", round(x), round(y), round(w), round(h))),
        ("points", format!("{:?}", shape.points.iter().map(|&(x, y)| (round(x), round(y))).collect::<Vec<_>>())),
        ("link", shape.link.clone()),
    ]
}

/// The names of what differs between two fingerprints.
fn changed(before: &[(&'static str, String)], after: &[(&'static str, String)]) -> Vec<&'static str> {
    before.iter().zip(after).filter(|(a, b)| a.1 != b.1).map(|(a, _)| a.0).collect()
}

/// Every shape on the board, fingerprinted, in order.
/// What an end is stuck to said by its layer rather than its name, which
/// two boards made alike name apart.
fn all(studio: &Studio) -> Vec<Vec<(&'static str, String)>> {
    all_of(&studio.doc.layers)
}

/// The same, of some layers: a board's, or a page read back from a file.
fn all_of(layers: &[Layer]) -> Vec<Vec<(&'static str, String)>> {
    let index_of = |id: u32| layers.iter().position(|layer| match &layer.kind {
        Kind::Shape(shape) => shape.id == id,
        Kind::Picture(picture) => picture.id == id,
    });
    layers.iter().filter_map(|layer| match &layer.kind {
        Kind::Shape(shape) => {
            let mut print = fingerprint(shape);
            let to = shape.stuck.map(|end| end.map(|(id, u, v)| (index_of(id), u, v)));
            if let Some(stuck) = print.iter_mut().find(|(name, _)| *name == "stuck") {
                stuck.1 = format!("{to:?}");
            }
            Some(print)
        }
        _ => None,
    }).collect()
}

/// A board in a look: a box with words, a box beside it joined to it by
/// an arrow, an oval, a note and words on their own. Gives the arrow.
fn furnished(look: u8) -> (Studio, usize) {
    let mut studio = board();
    studio.set_look(Some(look), None);
    for (from, to, words) in [((100.0, 300.0), (260.0, 372.0), "Proxy"), ((700.0, 250.0), (860.0, 322.0), "Auth")] {
        press_key(&mut studio, Key::Char('r'), Mods::default());
        pull(&mut studio, from, to, Mods::default());
        type_in(&mut studio, words);
        press_key(&mut studio, Key::Escape, Mods::default());
        press_key(&mut studio, Key::Escape, Mods::default());
    }
    let arrow = studio.join(0, 1);
    press_key(&mut studio, Key::Char('o'), Mods::default());
    pull(&mut studio, (100.0, 600.0), (220.0, 680.0), Mods::default());
    press_key(&mut studio, Key::Escape, Mods::default());
    press_key(&mut studio, Key::Escape, Mods::default());
    press_key(&mut studio, Key::Char('t'), Mods::default());
    tap(&mut studio, (600.0, 650.0));
    type_in(&mut studio, "Note to self");
    press_key(&mut studio, Key::Escape, Mods::default());
    press_key(&mut studio, Key::Escape, Mods::default());
    studio.selected.clear();
    (studio, arrow)
}

#[test]
fn in_each_look_an_arrow_pulled_from_a_box_is_shown_as_it_is_made() {
    // Into the open across and down, straight out to the right, and onto
    // the other box: the ghost while pulled, and the arrow once let go.
    for look in 0..=look::DIAGRAM {
        for (name, to) in [("open, across and down", (560.0, 520.0)), ("open, straight out", (560.0, 336.0)), ("onto the other box", (780.0, 290.0))] {
            let (mut studio, _) = furnished(look);
            let arrows = studio.doc.layers.len();
            let over = studio.to_window_page((180.0, 336.0));
            studio.pointer_moved(over.0, over.1, Mods::default());
            let right = studio.dot_spots(0)[1];
            studio.pointer_moved(right.0, right.1, Mods::default());
            studio.last_press.0 = 0;
            studio.pressed(1, 100_000);
            let to = studio.to_window_page(to);
            for step in 1..=12 {
                studio.pointer_moved(right.0 + (to.0 - right.0) * step / 12, right.1 + (to.1 - right.1) * step / 12, Mods::default());
            }
            let ghost = studio.pulled_arrow(0, studio.board.bind).expect("a ghost");
            studio.released();
            let made = studio.doc.layers[arrows..].iter().find_map(|layer| match &layer.kind {
                Kind::Shape(shape) if shape.tool == Tool::Arrow => Some(shape.clone()),
                _ => None,
            }).unwrap_or_else(|| panic!("look {look}, {name}: an arrow made"));
            let at = |shape: &Shape| (shape.elbow, shape.routed, shape.heads, shape.ends, shape.dashed, shape.dotted);
            assert_eq!(at(&ghost), at(&made), "look {look}, {name}: the ghost as made");
            let near = |a: &[(f32, f32)], b: &[(f32, f32)]| a.len() == b.len() && a.iter().zip(b).all(|(p, q)| (p.0 - q.0).abs() < 2.0 && (p.1 - q.1).abs() < 2.0);
            assert!(near(&ghost.path(), &made.path()), "look {look}, {name}: the ghost's way {:?} as made {:?}", ghost.points, made.points);
        }
    }
}

#[test]
fn in_each_look_each_part_of_the_panel_sets_what_it_says_and_nothing_else() {
    // For a box with words and for an arrow, picked out alone: what each
    // may change; anything else changed, or anything else on the board,
    // is a fault. A box's frame may grow for its words, and an arrow joined
    // to it go with it.
    let for_box: Vec<(Ctl, &[&str])> = vec![
        (Ctl::Color(2), &["color", "fill"]),
        (Ctl::Color(5), &["color", "fill"]),
        (Ctl::FillKind(0), &["filled", "fill", "hatch"]),
        (Ctl::FillKind(3), &["filled", "fill", "hatch"]),
        (Ctl::Dash(1), &["dashed", "dotted"]),
        (Ctl::Dash(2), &["dashed", "dotted"]),
        (Ctl::Size(0), &["font_px", "nominal", "frame", "points"]),
        (Ctl::Size(3), &["font_px", "nominal", "frame", "points"]),
        (Ctl::Words(2), &["font_px", "nominal", "frame", "points"]),
    ];
    let for_arrow: Vec<(Ctl, &[&str])> = vec![
        (Ctl::Color(2), &["color", "fill"]),
        (Ctl::Dash(1), &["dashed", "dotted"]),
        (Ctl::Size(3), &["font_px", "nominal"]),
        (Ctl::Head(0, 2), &["heads", "ends"]),
        (Ctl::Head(1, 4), &["heads", "ends"]),
        (Ctl::Route(0), &["routed", "elbow", "points", "frame", "step"]),
        (Ctl::Route(1), &["routed", "elbow", "points", "frame", "step"]),
        (Ctl::Route(2), &["routed", "elbow", "points", "frame", "step"]),
    ];
    for look in 0..=look::DIAGRAM {
        for (picked, controls) in [(0usize, &for_box), (usize::MAX, &for_arrow)] {
            for &(ctl, may) in controls.iter() {
                let (mut studio, arrow) = furnished(look);
                let index = if picked == usize::MAX { arrow } else { picked };
                studio.selected = vec![index];
                let before = all(&studio);
                studio.press_style(ctl);
                studio.board.stepping = None;
                let after = all(&studio);
                let shapes: Vec<usize> = (0..studio.doc.layers.len()).filter(|&at| matches!(studio.doc.layers[at].kind, Kind::Shape(_))).collect();
                for (order, (was, now)) in before.iter().zip(&after).enumerate() {
                    let at = shapes[order];
                    let differ = changed(was, now);
                    if at == index {
                        let stray: Vec<&str> = differ.iter().copied().filter(|name| !may.contains(name)).collect();
                        assert!(stray.is_empty(), "look {look}, {ctl:?} on layer {index}: also changed {stray:?}");
                    } else if at == arrow && index == 0 {
                        // The arrow joined to the box goes with its frame.
                        let stray: Vec<&str> = differ.iter().copied().filter(|name| !["points", "frame"].contains(name)).collect();
                        assert!(stray.is_empty(), "look {look}, {ctl:?}: the arrow joined also changed {stray:?}");
                    } else {
                        assert!(differ.is_empty(), "look {look}, {ctl:?} on layer {index}: layer {at} changed {differ:?}");
                    }
                }
            }
        }
    }
}

#[test]
fn a_look_changes_how_a_board_is_drawn_not_what_is_on_it_and_one_step_undoes_it() {
    // From each look to each other: the same shapes, where they were, in
    // the same colors and sizes. Only Diagram sets lettered words in Geist
    // and turns bowed arrows round corners, as it says; and one step back
    // has all as it was.
    for from in 0..=look::DIAGRAM {
        for to in 0..=look::DIAGRAM {
            if from == to {
                continue;
            }
            let (mut studio, _) = furnished(from);
            let before = all(&studio);
            studio.set_look(Some(to), None);
            let after = all(&studio);
            for (was, now) in before.iter().zip(&after) {
                let mut may: Vec<&str> = Vec::new();
                if to == look::DIAGRAM && from != look::DIAGRAM {
                    may.extend(["face", "routed", "elbow", "points", "frame"]);
                }
                let stray: Vec<&str> = changed(was, now).into_iter().filter(|name| !may.contains(name)).collect();
                assert!(stray.is_empty(), "look {from} to {to}: {stray:?} changed");
            }
            studio.undo();
            assert_eq!(all(&studio), before, "look {from} to {to}: one step undoes it");
        }
    }
}

#[test]
fn each_thing_done_on_a_board_is_one_step_to_undo_and_kept_in_the_file() {
    type Done = fn(&mut Studio, usize);
    let doings: Vec<(&str, Done)> = vec![
        ("a box drawn", |studio, _| {
            press_key(studio, Key::Char('r'), Mods::default());
            pull(studio, (400.0, 500.0), (520.0, 560.0), Mods::default());
            press_key(studio, Key::Escape, Mods::default());
            press_key(studio, Key::Escape, Mods::default());
        }),
        ("two joined", |studio, _| {
            studio.checkpoint();
            studio.join(0, 2);
        }),
        ("colored", |studio, _| {
            studio.selected = vec![0];
            studio.press_style(Ctl::Color(3));
        }),
        ("sized", |studio, _| {
            studio.selected = vec![0];
            studio.press_style(Ctl::Size(3));
        }),
        ("routed another way", |studio, arrow| {
            studio.selected = vec![arrow];
            let other = if studio.pages[0].look == look::DIAGRAM { 0 } else { 2 };
            studio.press_style(Ctl::Route(other));
        }),
        ("lined up", |studio, _| {
            studio.selected = vec![0, 1];
            let _ = studio.act(Action::Align(3));
        }),
        ("matched", |studio, _| {
            studio.selected = vec![0, 3];
            let _ = studio.act(Action::Match(2));
        }),
        ("on dark paper", |studio, _| studio.set_look(None, Some(1))),
        ("linked to the web", |studio, _| {
            studio.selected = vec![0];
            studio.board_act(Act::Link);
            type_in(studio, "example.com/a,b%20c");
            press_key(studio, Key::Enter, Mods::default());
        }),
        ("linked to another box", |studio, _| {
            studio.selected = vec![0];
            studio.board_act(Act::LinkTo);
            tap(studio, (780.0, 286.0));
        }),
        ("the next box by the keys", |studio, _| {
            studio.selected = vec![0];
            press_key(studio, Key::Right, Mods { ctrl: true, ..Mods::default() });
            type_in(studio, "Next");
            press_key(studio, Key::Escape, Mods::default());
        }),
        ("deleted", |studio, _| {
            studio.selected = vec![1];
            press_key(studio, Key::Delete, Mods::default());
        }),
    ];
    for look in 0..=look::DIAGRAM {
        for (name, doing) in &doings {
            let (mut studio, arrow) = furnished(look);
            let before = all(&studio);
            let paper = (studio.pages[0].look, studio.pages[0].ground);
            doing(&mut studio, arrow);
            let after = all(&studio);
            assert!(after != before || (studio.pages[0].look, studio.pages[0].ground) != paper, "look {look}, {name}: something done");
            // Kept in the file as it is.
            let back = read_pages(&write_pages(&studio.gathered())).unwrap();
            let kept = all_of(&back[0].loose.layers);
            assert_eq!(kept, after, "look {look}, {name}: the file keeps it");
            // One step back: as it was.
            studio.undo();
            assert_eq!(all(&studio), before, "look {look}, {name}: one step undoes it");
            assert_eq!((studio.pages[0].look, studio.pages[0].ground), paper, "look {look}, {name}: and the paper");
        }
    }
}

/// A box put on a board as its box tool would draw it.
fn put(studio: &mut Studio, (x, y, w, h): (f32, f32, f32, f32), words: &str) -> usize {
    let mut shape = Shape::new(Tool::Rect, INK, LINE, (x, y));
    studio.board_dress(&mut shape);
    shape.points[1] = (x + w, y + h);
    (shape.text, shape.base) = (words.into(), (w, h));
    studio.doc.layers.push(Layer::shape(shape));
    studio.changed();
    studio.doc.layers.len() - 1
}

#[test]
fn in_each_look_what_is_guessed_is_kept_where_it_was_shown_and_drawn_the_look_s_way() {
    // A third box of a row, the next of a chain, and another child of a
    // tree: what Tab keeps where the ghost showed it, its arrows on the way
    // the ghost showed and going the way the look's arrows go.
    type Setup = fn(&mut Studio);
    let setups: [(&str, Setup); 3] = [
        ("a row", |studio| {
            put(studio, (100.0, 100.0, 140.0, 60.0), "Read");
            put(studio, (300.0, 100.0, 140.0, 60.0), "Parse");
        }),
        ("a chain", |studio| {
            let a = put(studio, (100.0, 100.0, 120.0, 60.0), "API");
            let b = put(studio, (300.0, 100.0, 120.0, 60.0), "Queue");
            let c = put(studio, (500.0, 100.0, 120.0, 60.0), "Worker");
            studio.join(a, b);
            studio.join(b, c);
            put(studio, (500.0, 300.0, 120.0, 60.0), "Store");
        }),
        ("a tree", |studio| {
            let parent = put(studio, (100.0, 200.0, 120.0, 60.0), "Root");
            let one = put(studio, (320.0, 100.0, 120.0, 60.0), "One");
            let two = put(studio, (320.0, 200.0, 120.0, 60.0), "Two");
            studio.join(parent, one);
            studio.join(parent, two);
        }),
    ];
    for look in 0..=look::DIAGRAM {
        for (name, setup) in setups {
            let mut studio = board();
            studio.set_look(Some(look), None);
            setup(&mut studio);
            studio.changed();
            studio.guess(true);
            let ghost = studio.board.ghost.clone().unwrap_or_else(|| panic!("look {look}, {name}: something guessed"));
            let shown: Vec<Shape> = ghost.layers.iter().filter_map(|layer| match &layer.kind {
                Kind::Shape(shape) => Some(shape.clone()),
                _ => None,
            }).collect();
            let start = studio.doc.layers.len();
            assert!(studio.keep_ghost(false), "look {look}, {name}: kept");
            let kept: Vec<Shape> = studio.doc.layers[start..].iter().filter_map(|layer| match &layer.kind {
                Kind::Shape(shape) => Some(shape.clone()),
                _ => None,
            }).collect();
            assert_eq!(kept.len(), shown.len(), "look {look}, {name}: all of it kept");
            let route = studio.board_route_kind();
            for (ghost, kept) in shown.iter().zip(&kept) {
                match kept.tool {
                    Tool::Arrow | Tool::Line => {
                        assert_eq!((kept.routed, kept.elbow), (route == 0, route == 2), "look {look}, {name}: its arrow the look's way");
                        let near = |a: &[(f32, f32)], b: &[(f32, f32)]| a.len() == b.len() && a.iter().zip(b).all(|(p, q)| (p.0 - q.0).abs() < 2.0 && (p.1 - q.1).abs() < 2.0);
                        assert!(near(&ghost.path(), &kept.path()), "look {look}, {name}: the arrow shown {:?} as kept {:?}", ghost.points, kept.points);
                    }
                    _ => assert_eq!(ghost.frame(), kept.frame(), "look {look}, {name}: where it was shown"),
                }
            }
        }
    }
}

#[test]
fn in_each_look_a_box_let_go_on_an_arrow_does_what_it_said_it_would() {
    // Dragged onto the arrow, then let go: "Between" puts it between the
    // two the arrow joined; "Branch beside" joins it from where the arrow
    // comes, the arrow kept.
    for look in 0..=look::DIAGRAM {
        for (name, down) in [("onto it", 13), ("beside it", 11)] {
            let mut studio = board();
            studio.set_look(Some(look), None);
            let a = put(&mut studio, (100.0, 300.0, 160.0, 72.0), "A");
            let b = put(&mut studio, (700.0, 300.0, 160.0, 72.0), "B");
            let c = put(&mut studio, (400.0, 60.0, 120.0, 60.0), "C");
            studio.join(a, b);
            studio.selected.clear();
            let (a, b, c) = (studio.id_of(a), studio.id_of(b), studio.id_of(c));
            let start = studio.to_window_page((460.0, 90.0));
            studio.pointer_moved(start.0, start.1, Mods::default());
            studio.last_press.0 = 0;
            studio.pressed(1, 100_000);
            for step in 1..=down {
                studio.pointer_moved(start.0, start.1 + step * 20, Mods::default());
            }
            let said = studio.board.drop.unwrap_or_else(|| panic!("look {look}, {name}: something said"));
            studio.released();
            let joined: Vec<(u32, u32)> = studio.links().into_iter().map(|(_, from, to)| (from, to)).collect();
            match said.split {
                true => assert!(joined.contains(&(a, c)) && joined.contains(&(c, b)) && !joined.contains(&(a, b)), "look {look}, {name}: between, as said: {joined:?}"),
                false => assert!(joined.contains(&(a, c)) && joined.contains(&(a, b)), "look {look}, {name}: a branch, as said: {joined:?}"),
            }
        }
    }
}

/// All of the window as drawn, against all of it drawn from nothing: the
/// same, to a shade, the paper's dots apart.
fn as_if_drawn_afresh(studio: &mut Studio, what: &str) {
    // What is said for a moment fades by the clock between the two
    // drawings: none, for them to be alike.
    studio.note = None;
    let mut kept = vec![0u32; (studio.width * studio.height) as usize];
    studio.draw(&mut kept);
    let mut fresh = vec![0u32; kept.len()];
    (studio.shown.clear(), studio.shown_page.clear());
    studio.draw(&mut fresh);
    let (paper, dots) = studio.paper();
    let ground = |c: u32| matches!(c & 0xFF_FFFF, c if c == paper || c == dots);
    let far = |a: u32, b: u32| !(ground(a) && ground(b)) && [0, 8, 16].iter().any(|shift| ((a >> shift & 0xFF) as i32 - (b >> shift & 0xFF) as i32).abs() > 2);
    let w = studio.width as usize;
    let off: Vec<(usize, usize)> = kept.iter().zip(&fresh).enumerate().filter(|&(_, (&a, &b))| far(a, b)).map(|(at, _)| (at % w, at / w)).collect();
    assert!(off.is_empty(), "{what}: {} pixels of the window as drawn differ from it drawn afresh, from {:?}", off.len(), &off[..off.len().min(4)]);
}

#[test]
fn in_each_look_what_the_panel_changes_is_drawn_as_if_afresh() {
    let controls = [Ctl::Color(4), Ctl::FillKind(3), Ctl::FillKind(0), Ctl::Dash(2), Ctl::Size(3), Ctl::Size(0), Ctl::Words(2), Ctl::Route(2), Ctl::Route(0), Ctl::Head(1, 3)];
    for look in 0..=look::DIAGRAM {
        let (mut studio, arrow) = furnished(look);
        let mut out = vec![0u32; (studio.width * studio.height) as usize];
        studio.draw(&mut out);
        for ctl in controls {
            for picked in [vec![0], vec![arrow], vec![0, 1, arrow]] {
                studio.selected = picked.clone();
                studio.press_style(ctl);
                studio.board.stepping = None;
                as_if_drawn_afresh(&mut studio, &format!("look {look}, {ctl:?} on {picked:?}"));
            }
        }
    }
}

#[test]
fn a_look_or_paper_hovered_shows_what_choosing_it_would_and_moving_off_puts_back_all_as_it_was() {
    use crate::studio::menu::Item;
    for look in 0..=look::DIAGRAM {
        for (list, kind, rows) in [("Theme", MenuKind::Looks, 4usize), ("Paper", MenuKind::Grounds, 3)] {
            for row in 0..rows {
                let (mut studio, _) = furnished(look);
                let mut before = vec![0u32; (studio.width * studio.height) as usize];
                studio.draw(&mut before);
                let shapes_before = all(&studio);
                // Hovered in the logo's menu.
                let logo = studio.control_rect(Control::Home).unwrap();
                click(&mut studio, logo.center());
                let at = studio.menu_items(MenuKind::Main).iter().position(|item| matches!(item, Item::Entry { says, .. } if says == list)).unwrap();
                let rect = studio.control_rect(Control::MenuRow(0, at as u16)).unwrap();
                studio.pointer_moved(rect.center().0, rect.center().1, Mods::default());
                let entries: Vec<usize> = studio.menu_items(kind).iter().enumerate().filter(|(_, item)| matches!(item, Item::Entry { .. })).map(|(at, _)| at).collect();
                let rect = studio.control_rect(Control::MenuRow(1, entries[row] as u16)).unwrap();
                studio.pointer_moved(rect.center().0, rect.center().1, Mods::default());
                let hovered = (studio.pages[0].look, studio.pages[0].ground, all(&studio));
                // Moved off and let go: all as it was, drawn the same.
                press_key(&mut studio, Key::Escape, Mods::default());
                press_key(&mut studio, Key::Escape, Mods::default());
                assert_eq!((studio.pages[0].look, studio.pages[0].ground), (look, 0), "look {look}, {list} row {row}: put back");
                assert_eq!(all(&studio), shapes_before, "look {look}, {list} row {row}: the shapes put back");
                let mut after = vec![0u32; before.len()];
                studio.draw(&mut after);
                let w = studio.width as usize;
                let spots: Vec<(usize, usize)> = before.iter().zip(&after).enumerate().filter(|(_, (a, b))| a != b).map(|(at, _)| (at % w, at / w)).collect();
                let off = spots.len();
                let bound = spots.iter().fold((usize::MAX, usize::MAX, 0, 0), |(l, t, r, b), &(x, y)| (l.min(x), t.min(y), r.max(x), b.max(y)));
                assert!(off == 0, "look {look}, {list} row {row}: {off} pixels not put back, within {bound:?}; says {:?}", studio.note.as_ref().map(|(says, _)| says.clone()));
                // Chosen: as it was shown hovered.
                match kind {
                    MenuKind::Looks => studio.set_look(Some(row as u8), None),
                    _ => studio.set_look(None, Some(row as u8)),
                }
                assert_eq!((studio.pages[0].look, studio.pages[0].ground, all(&studio)), hovered, "look {look}, {list} row {row}: chosen as shown");
            }
        }
    }
}

#[test]
fn a_key_does_what_the_panel_or_the_menu_does_for_it() {
    for look in 0..=look::DIAGRAM {
        // C then a number: the color of that swatch, as the panel's.
        for (key, swatch) in [('1', 0u8), ('3', 2), ('7', 6)] {
            let (mut by_key, _) = furnished(look);
            let (mut by_panel, _) = furnished(look);
            by_key.selected = vec![0];
            by_panel.selected = vec![0];
            press_key(&mut by_key, Key::Char('c'), Mods::default());
            press_key(&mut by_key, Key::Char(key), Mods::default());
            by_panel.press_style(Ctl::Color(swatch));
            let differ: Vec<(usize, Vec<&str>)> = all(&by_key).iter().zip(all(&by_panel).iter()).enumerate().map(|(at, (a, b))| (at, changed(a, b))).filter(|(_, d)| !d.is_empty()).collect();
            assert!(differ.is_empty(), "look {look}: C {key} as the panel's swatch {swatch}: {differ:?} {:?} {:?}", all(&by_key)[0][..3].to_vec(), all(&by_panel)[0][..3].to_vec());
        }
        // Ctrl+Shift+C: what the menu's Copy as PNG copies, of one board.
        let (mut studio, _) = furnished(look);
        studio.selected = vec![0];
        press_key(&mut studio, Key::Char('C'), Mods { ctrl: true, shift: true, ..Mods::default() });
        let by_key = studio.copied_picture.take();
        studio.run_menu(crate::studio::menu::MenuAct::Board(Act::CopyAs(0)));
        assert!(by_key.is_some() && by_key == studio.copied_picture, "look {look}: the same picture copied");
    }
}

#[test]
fn in_each_look_each_export_choice_changes_the_picture_as_it_says_and_nothing_on_the_board() {
    for look in 0..=look::DIAGRAM {
        let (mut studio, _) = furnished(look);
        let (file, undos) = (write_pages(&studio.gathered()), studio.undo.len());
        let copy = |studio: &mut Studio| {
            studio.copy_board(0, true);
            let png = studio.copied_picture.take().expect("copied");
            let picture = crate::convert::decode(&png).expect("a PNG");
            (picture.width, picture.height, picture.pixels[0], crate::output::board_of(&png))
        };
        let (w, h, corner, inside) = copy(&mut studio);
        assert!(inside.is_some(), "look {look}: the board in it");
        // Each choice: what it changes of the picture; the board in it
        // the same throughout.
        let changes: [(&str, fn(&mut export::Choices)); 5] = [
            ("1x", |choices| choices.times = 1),
            ("white", |choices| choices.ground = 1),
            ("none", |choices| choices.ground = 2),
            ("room", |choices| choices.room = 64),
            ("not kept", |choices| choices.keep = false),
        ];
        for (name, change) in changes {
            let was = studio.board.choices;
            change(&mut studio.board.choices);
            let (cw, ch, ccorner, cinside) = copy(&mut studio);
            let size = match name {
                "1x" => (w / 2, h / 2),
                "room" => (w + 2 * 2 * 32, h + 2 * 2 * 32),
                _ => (w, h),
            };
            assert!((cw - size.0).abs() <= 1 && (ch - size.1).abs() <= 1, "look {look}, {name}: {cw}x{ch}, not {size:?}");
            let ground = match name {
                "white" => 0xFFFF_FFFF,
                "none" => 0,
                _ => corner,
            };
            assert_eq!(ccorner, ground, "look {look}, {name}: its ground");
            assert_eq!(cinside.is_some(), name != "not kept", "look {look}, {name}: the board in it or not");
            if let (Some(a), Some(b)) = (&inside, &cinside) {
                assert!(a == b, "look {look}, {name}: the same board in it");
            }
            studio.board.choices = was;
        }
        assert!(write_pages(&studio.gathered()) == file && studio.undo.len() == undos, "look {look}: nothing on the board changed");
    }
}

#[test]
fn in_each_look_ctrl_and_an_arrow_makes_the_next_box_where_it_showed_joined_the_look_s_way() {
    // From a box picked out, and from one typed into, each way: a box like
    // it where Ctrl held showed it, clear of the rest, joined from it by
    // an arrow going the way the look's arrows go; one step to undo, and
    // kept in the file.
    let ways = [Key::Up, Key::Right, Key::Down, Key::Left];
    for look in 0..=look::DIAGRAM {
        for (way, key) in ways.into_iter().enumerate() {
            for typing in [false, true] {
                let (mut studio, _) = furnished(look);
                studio.selected = vec![0];
                if typing {
                    press_key(&mut studio, Key::Enter, Mods::default());
                    assert_eq!(studio.editing, Some(0));
                }
                let what = format!("look {look}, {key:?}, typing {typing}");
                let before = all(&studio);
                studio.ctrl_held(true);
                studio.board.ctrl = Some(std::time::Instant::now() - std::time::Duration::from_secs(1));
                let shown = studio.flow_ghosts().unwrap_or_else(|| panic!("{what}: shown"))[way];
                let count = studio.doc.layers.len();
                press_key(&mut studio, key, Mods { ctrl: true, ..Mods::default() });
                assert_eq!(studio.doc.layers.len(), count + 2, "{what}: a box and an arrow");
                let made = studio.picked().unwrap();
                assert_eq!(studio.editing, Some(made), "{what}: typed into");
                assert_eq!(studio.box_frame(made), shown, "{what}: where it showed");
                // Like the box it is from, but for its words and where it is.
                let (from, new) = (fingerprint(&shape(&studio, 0)), fingerprint(&shape(&studio, made)));
                let differ = changed(&from, &new);
                assert!(differ.iter().all(|name| ["text", "frame", "points"].contains(name)), "{what}: also differs in {differ:?}");
                // Clear of every other box.
                let (x, y, w, h) = studio.box_frame(made);
                for other in (0..studio.doc.layers.len()).filter(|&index| index != made && studio.is_node(index)) {
                    let o = studio.box_frame(other);
                    assert!(o.0 >= x + w || o.0 + o.2 <= x || o.1 >= y + h || o.1 + o.3 <= y, "{what}: over layer {other}");
                }
                // Joined from it, the look's way.
                let arrow = studio.doc.layers.iter().rev().find_map(|layer| match &layer.kind {
                    Kind::Shape(shape) if shape.sticks() => Some(shape.clone()),
                    _ => None,
                }).unwrap();
                let ends = arrow.stuck.map(|end| end.map(|(id, ..)| id));
                assert_eq!(ends, [Some(studio.id_of(0)), Some(studio.id_of(made))], "{what}: joined");
                let route = studio.board_route_kind();
                assert_eq!((arrow.routed, arrow.elbow), (route == 0, route == 2), "{what}: the look's way");
                type_in(&mut studio, "Next");
                press_key(&mut studio, Key::Escape, Mods::default());
                let after = all(&studio);
                let back = read_pages(&write_pages(&studio.gathered())).unwrap();
                assert_eq!(all_of(&back[0].loose.layers), after, "{what}: the file keeps it");
                // One step, its words with it, as in any box made.
                studio.undo();
                assert_eq!(all(&studio), before, "{what}: one step undoes it");
            }
        }
    }
}

#[test]
fn in_each_look_a_board_copied_as_a_png_pastes_back_as_the_same_shapes_in_one_step() {
    for look in 0..=look::DIAGRAM {
        let (mut studio, _) = furnished(look);
        let was = all(&studio);
        let count = studio.doc.layers.len();
        press_key(&mut studio, Key::Char('C'), Mods { ctrl: true, shift: true, ..Mods::default() });
        let png = studio.copied_picture.take().unwrap_or_else(|| panic!("look {look}: copied"));
        // Pasted where the view is, moved well off it, so that nothing
        // pasted is in the way of an arrow already there.
        studio.pan.0 -= 3000.0;
        let undos = studio.undo.len();
        studio.pasted(Some(png), None);
        assert_eq!((studio.doc.layers.len(), studio.undo.len()), (count * 2, undos + 1), "look {look}: all of it again, one step to undo");
        assert_eq!(all(&studio)[..was.len()], was[..], "look {look}: what was there as it was");
        // The same shapes, joined the same, moved together: what differs
        // is where, by one step across and down for all.
        let pasted = all_of(&studio.doc.layers[count..]);
        let frame = |print: &[(&'static str, String)]| print.iter().find(|(name, _)| *name == "frame").map(|(_, frame)| frame.split(' ').map(|v| v.parse::<f32>().unwrap()).collect::<Vec<_>>()).unwrap();
        let by = (frame(&pasted[0])[0] - frame(&was[0])[0], frame(&pasted[0])[1] - frame(&was[0])[1]);
        for (at, (a, b)) in was.iter().zip(&pasted).enumerate() {
            let differ: Vec<&str> = changed(a, b).into_iter().filter(|name| !matches!(*name, "frame" | "points")).collect();
            assert!(differ.is_empty(), "look {look}, shape {at}: {differ:?}");
            let (fa, fb) = (frame(a), frame(b));
            assert!((fb[0] - fa[0] - by.0).abs() <= 0.5 && (fb[1] - fa[1] - by.1).abs() <= 0.5 && fa[2..] == fb[2..], "look {look}, shape {at}: moved with the rest {fa:?} {fb:?}");
        }
        assert_eq!(studio.selected, (count..count * 2).collect::<Vec<_>>(), "look {look}: picked out");
        studio.undo();
        assert_eq!(all(&studio), was, "look {look}: undone, as it was");
    }
}

#[test]
fn in_each_look_a_link_wears_its_badge_is_kept_in_the_file_and_goes_where_it_says() {
    for look in 0..=look::DIAGRAM {
        let (mut studio, _) = furnished(look);
        studio.selected = vec![0];
        studio.board_act(Act::LinkTo);
        tap(&mut studio, (780.0, 286.0));
        let linked = format!("#{}", studio.id_of(1));
        assert_eq!(shape(&studio, 0).link, linked, "look {look}");
        // Its badge, drawn and pressed where it is.
        studio.selected.clear();
        let badge = studio.control_rect(Control::Board(Ctl::LinkBadge(studio.id_of(0)))).unwrap_or_else(|| panic!("look {look}: a badge"));
        let mut out = vec![0u32; (studio.width * studio.height) as usize];
        studio.draw(&mut out);
        let (cx, cy) = badge.center();
        let pixel = out[((cy + 6) * studio.width + cx) as usize];
        assert!((pixel >> 8 & 0xFF) > (pixel >> 16 & 0xFF) + 40, "look {look}: green, {pixel:06X}");
        // Kept in the file, and through it the same.
        let back = read_pages(&write_pages(&studio.gathered())).unwrap();
        assert_eq!(all_of(&back[0].loose.layers), all(&studio), "look {look}: the file keeps it");
        // Its badge pressed, the other box picked out, in view.
        studio.pan = (-3000.0, -3000.0);
        studio.pointer_moved(cx, cy, Mods::default());
        let badge = studio.control_rect(Control::Board(Ctl::LinkBadge(studio.id_of(0))));
        assert!(badge.is_none(), "look {look}: out of view, no badge");
        studio.pan = (0.0, 0.0);
        click(&mut studio, (cx, cy));
        assert_eq!(studio.selected, vec![1], "look {look}");
    }
}

#[test]
fn in_each_look_what_is_found_is_marked_on_its_own_words() {
    // Words in a box and an arrow found in each look, each mark on the
    // letters it marks, within its shape's own words.
    for look in 0..=look::DIAGRAM {
        let (mut studio, arrow) = furnished(look);
        if let Kind::Shape(shape) = &mut studio.doc.layers[arrow].kind {
            shape.text = "via auth".into();
            shape.touch();
        }
        studio.changed();
        press_key(&mut studio, Key::Char('f'), Mods { ctrl: true, ..Mods::default() });
        type_in(&mut studio, "AUTH");
        let found = studio.find_all();
        assert_eq!(found.len(), 2, "look {look}: the box and the arrow");
        for found in found {
            let pieces = studio.found_pieces(found);
            assert_eq!(pieces.len(), 1, "look {look}");
            let (x, y, w, h) = pieces[0];
            let words = studio.doc.layers[found.index].bounds(&mut studio.text);
            assert!(w > 10.0 && h > 8.0 && x >= words.0 - 1.0 && y >= words.1 - 1.0 && x + w <= words.0 + words.2 + 1.0, "look {look}: {pieces:?} in {words:?}");
        }
    }
}

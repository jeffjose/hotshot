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
    ]
}

/// The names of what differs between two fingerprints.
fn changed(before: &[(&'static str, String)], after: &[(&'static str, String)]) -> Vec<&'static str> {
    before.iter().zip(after).filter(|(a, b)| a.1 != b.1).map(|(a, _)| a.0).collect()
}

/// Every shape on the board, fingerprinted, in order.
fn all(studio: &Studio) -> Vec<Vec<(&'static str, String)>> {
    studio.doc.layers.iter().filter_map(|layer| match &layer.kind {
        Kind::Shape(shape) => Some(fingerprint(shape)),
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
            drop(studio.act(Action::Align(3)));
        }),
        ("matched", |studio, _| {
            studio.selected = vec![0, 3];
            drop(studio.act(Action::Match(2)));
        }),
        ("on dark paper", |studio, _| studio.set_look(None, Some(1))),
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
            let kept: Vec<Vec<(&'static str, String)>> = back[0].loose.layers.iter().filter_map(|layer| match &layer.kind {
                Kind::Shape(shape) => Some(fingerprint(shape)),
                _ => None,
            }).collect();
            assert_eq!(kept, after, "look {look}, {name}: the file keeps it");
            // One step back: as it was.
            studio.undo();
            assert_eq!(all(&studio), before, "look {look}, {name}: one step undoes it");
            assert_eq!((studio.pages[0].look, studio.pages[0].ground), paper, "look {look}, {name}: and the paper");
        }
    }
}

//! Pointers hotshot draws for itself, which the cursor themes have no good
//! one of: the one for turning what is picked out, a curved arrow with a
//! head at each end, as Figma's is, bowed out the way the corner it is at
//! points.

/// How wide and high a drawn pointer is, and where in it its point is.
pub const SIZE: i32 = 32;
pub const HOT: i32 = SIZE / 2;

/// How many ways the turning pointer is drawn, one each eighth of a turn.
pub const TURNS: u8 = 8;

/// The way the turning pointer's bow faces for the way a corner points:
/// counted in eighths of a turn clockwise from straight right, nearest.
pub fn turn_way(dx: f32, dy: f32) -> u8 {
    let eighths = (dy.atan2(dx) / std::f32::consts::FRAC_PI_4).round() as i32;
    eighths.rem_euclid(i32::from(TURNS)) as u8
}

/// The turning pointer bowed out toward `way` eighths of a turn clockwise
/// from straight right: each pixel as how opaque it is in its top byte and
/// its colors times that, as the Render extension wants them. Black, with
/// an edge of white to show on anything.
pub fn turn_image(way: u8) -> Vec<u32> {
    let facing = f32::from(way % TURNS) * std::f32::consts::FRAC_PI_4;
    let (dx, dy) = (facing.cos(), facing.sin());
    // The arc is about a point behind the point of the pointer, so that it
    // bows out past it, and spans a quarter of a turn or so.
    let (radius, span) = (9.0f32, 0.95f32);
    let centre = (HOT as f32 - dx * 5.0, HOT as f32 - dy * 5.0);
    let ends = [facing - span, facing + span];
    // Where each end of the arc is, and the way along it there, outward.
    let heads: Vec<((f32, f32), (f32, f32))> = ends
        .iter()
        .zip([-1.0f32, 1.0])
        .map(|(&angle, side)| {
            let tip = (centre.0 + radius * angle.cos(), centre.1 + radius * angle.sin());
            let along = (-angle.sin() * side, angle.cos() * side);
            (tip, along)
        })
        .collect();
    // How far a point is from the arc's line, or inside a head.
    let distance = |x: f32, y: f32| {
        let (rx, ry) = (x - centre.0, y - centre.1);
        let angle = ry.atan2(rx);
        let off = (angle - facing + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        let line = if off.abs() <= span {
            (rx.hypot(ry) - radius).abs() - 1.0
        } else {
            f32::MAX
        };
        heads.iter().fold(line, |nearest, &((tx, ty), (ax, ay))| {
            // A head: a triangle pointing along the arc, out from its end.
            let (px, py) = (x - tx, y - ty);
            let forward = px * ax + py * ay;
            let across = (px * -ay + py * ax).abs();
            let head = (forward - 4.0).max(across - (4.0 - forward) * 0.6).max(-forward - 1.0);
            nearest.min(head)
        })
    };
    let mut pixels = vec![0u32; (SIZE * SIZE) as usize];
    for y in 0..SIZE {
        for x in 0..SIZE {
            // Four samples a side, for smooth edges.
            let (mut ink, mut edge) = (0.0f32, 0.0f32);
            for sy in 0..4 {
                for sx in 0..4 {
                    let far = distance(x as f32 + (sx as f32 + 0.5) / 4.0, y as f32 + (sy as f32 + 0.5) / 4.0);
                    ink += f32::from(u8::from(far <= 0.0));
                    edge += f32::from(u8::from(far <= 1.4));
                }
            }
            let (ink, edge) = (ink / 16.0, edge / 16.0);
            // Black over white: the white shows where there is no black.
            let alpha = edge.max(ink);
            let white = (edge - ink).max(0.0);
            let level = (white * 255.0).round() as u32;
            let a = (alpha * 255.0).round() as u32;
            pixels[(y * SIZE + x) as usize] = a << 24 | level << 16 | level << 8 | level;
        }
    }
    pixels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_turning_pointer_bows_out_the_way_its_corner_points() {
        assert_eq!((turn_way(1.0, 0.0), turn_way(-1.0, -1.0), turn_way(1.0, 1.0), turn_way(0.0, -1.0)), (0, 5, 1, 6));
        // Bowed to the top left, there is ink up there and none at the
        // bottom right.
        let image = turn_image(5);
        let shown = |x: i32, y: i32| image[(y * SIZE + x) as usize] >> 24;
        let top_left: u32 = (4..16).flat_map(|y| (4..16).map(move |x| (x, y))).map(|(x, y)| shown(x, y)).sum();
        let bottom_right: u32 = (20..30).flat_map(|y| (20..30).map(move |x| (x, y))).map(|(x, y)| shown(x, y)).sum();
        assert!(top_left > 1000 && bottom_right == 0, "{top_left} {bottom_right}");
    }
}

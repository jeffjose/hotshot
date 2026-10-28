//! The wheel and the trackpad as smoothly as they turn: XInput 2.1 says
//! how far a scrolling axis of a device has moved, in parts of a notch,
//! where the old buttons 4 to 7 say only that a whole notch has gone by.
//! A trackpad, or a wheel that turns finely, so zooms and pans as
//! smoothly as it is moved. Where the server has no XInput 2.1, the old
//! buttons are heard as ever.
//!
//! Asked for, XInput's pointer events take the place of the old ones on
//! the window: its movements and presses all come this way, turned into
//! the old kind for the rest of hotshot, and a notch of a wheel the
//! server has already told as movement along an axis, and then again as
//! a press it makes up, is heard once.

use std::collections::HashMap;

use x11rb::protocol::xinput::{self, ConnectionExt as _};
use x11rb::protocol::xproto;
use x11rb::rust_connection::RustConnection;

/// A scrolling axis of a device: which of its valuators it is, whether
/// up and down or across, how far along it a notch is, and where along
/// it it was last, to say how far it has moved since.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Axis {
    pub number: u16,
    pub down: bool,
    pub notch: f64,
    pub last: Option<f64>,
}

/// The scrolling axes of each device the pointer has been moved by.
pub(super) struct Smooth {
    devices: HashMap<u16, Vec<Axis>>,
}

/// What XInput told, as the rest of hotshot hears it: a pointer's
/// movement or press of the old kind; so many notches right and down a
/// scrolling axis moved, with where the pointer was and the keys and
/// buttons held; or nothing to hear.
pub(super) enum Heard {
    Core(x11rb::protocol::Event),
    Scroll(i32, i32, u16, f32, f32),
    Nothing,
}

/// The pointer's events asked for from XInput 2.1 on the window, if the
/// server has it.
pub(super) fn start(conn: &RustConnection, window: xproto::Window) -> Option<Smooth> {
    let version = conn.xinput_xi_query_version(2, 1).ok()?.reply().ok()?;
    if (version.major_version, version.minor_version) < (2, 1) {
        return None;
    }
    let wanted = xinput::XIEventMask::MOTION | xinput::XIEventMask::BUTTON_PRESS | xinput::XIEventMask::BUTTON_RELEASE | xinput::XIEventMask::ENTER | xinput::XIEventMask::DEVICE_CHANGED;
    // All the master pointers: the one there is, almost always.
    let mask = xinput::EventMask { deviceid: 1, mask: vec![wanted] };
    conn.xinput_xi_select_events(window, &[mask]).ok()?.check().ok()?;
    Some(Smooth { devices: HashMap::new() })
}

/// A number in 32.32 fixed point.
fn fixed(value: xinput::Fp3232) -> f64 {
    f64::from(value.integral) + f64::from(value.frac) / 4_294_967_296.0
}

/// A coordinate in 16.16 fixed point, to the pixel it is in.
fn pixel(value: xinput::Fp1616) -> i16 {
    (value >> 16) as i16
}

/// The scrolling axes among a device's classes.
fn axes(classes: &[xinput::DeviceClass]) -> Vec<Axis> {
    classes
        .iter()
        .filter_map(|class| match &class.data {
            xinput::DeviceClassData::Scroll(scroll) if fixed(scroll.increment) != 0.0 => Some(Axis { number: scroll.number, down: scroll.scroll_type == xinput::ScrollType::VERTICAL, notch: fixed(scroll.increment), last: None }),
            _ => None,
        })
        .collect()
}

/// How many notches right and down the scrolling axes moved, from the
/// valuators an event names and their values, one after another for
/// each bit set; where an axis was is kept for the next. An axis heard
/// of for the first time since the pointer came in has moved nowhere,
/// as where it was before is not known.
pub(super) fn scrolled(axes: &mut [Axis], mask: &[u32], values: &[xinput::Fp3232]) -> (f32, f32) {
    let (mut right, mut down) = (0.0, 0.0);
    let mut values = values.iter();
    for number in 0..mask.len() as u16 * 32 {
        if mask[usize::from(number / 32)] & (1 << (number % 32)) == 0 {
            continue;
        }
        let Some(&value) = values.next() else { break };
        let Some(axis) = axes.iter_mut().find(|axis| axis.number == number) else { continue };
        let now = fixed(value);
        if let Some(last) = axis.last {
            let notches = ((now - last) / axis.notch) as f32;
            if axis.down {
                down += notches;
            } else {
                right += notches;
            }
        }
        axis.last = Some(now);
    }
    (right, down)
}

impl Smooth {
    /// The scrolling axes of a device, asked of the server the first time.
    fn device(&mut self, conn: &RustConnection, id: u16) -> &mut Vec<Axis> {
        self.devices.entry(id).or_insert_with(|| {
            let info = conn.xinput_xi_query_device(id).ok().and_then(|cookie| cookie.reply().ok());
            info.map(|info| info.infos.iter().flat_map(|device| axes(&device.classes)).collect()).unwrap_or_default()
        })
    }

    /// Where every axis was forgotten: what it moved while the pointer
    /// was elsewhere is no part of a scroll here.
    fn forget(&mut self) {
        self.devices.values_mut().flatten().for_each(|axis| axis.last = None);
    }

    /// An event from XInput, as the rest of hotshot hears it.
    pub(super) fn hear(&mut self, conn: &RustConnection, event: x11rb::protocol::Event) -> Heard {
        use x11rb::protocol::Event;
        match event {
            Event::XinputMotion(motion) => {
                let (right, down) = scrolled(self.device(conn, motion.sourceid), &motion.valuator_mask, &motion.axisvalues);
                let state = (motion.mods.effective & 0xFF) as u16 | (motion.button_mask.first().copied().unwrap_or(0) << 7 & 0x1F00) as u16;
                if right != 0.0 || down != 0.0 {
                    return Heard::Scroll(i32::from(pixel(motion.event_x)), i32::from(pixel(motion.event_y)), state, right, down);
                }
                Heard::Core(Event::MotionNotify(xproto::MotionNotifyEvent {
                    response_type: xproto::MOTION_NOTIFY_EVENT,
                    detail: xproto::Motion::NORMAL,
                    sequence: motion.sequence,
                    time: motion.time,
                    root: motion.root,
                    event: motion.event,
                    child: motion.child,
                    root_x: pixel(motion.root_x),
                    root_y: pixel(motion.root_y),
                    event_x: pixel(motion.event_x),
                    event_y: pixel(motion.event_y),
                    state: state.into(),
                    same_screen: true,
                }))
            }
            Event::XinputButtonPress(press) | Event::XinputButtonRelease(press) => {
                // A notch the server made up for a wheel it has told as
                // movement along an axis already.
                if (4..=7).contains(&press.detail) && u32::from(press.flags) & u32::from(xinput::PointerEventFlags::POINTER_EMULATED) != 0 {
                    return Heard::Nothing;
                }
                let state = (press.mods.effective & 0xFF) as u16 | (press.button_mask.first().copied().unwrap_or(0) << 7 & 0x1F00) as u16;
                let core = xproto::ButtonPressEvent {
                    response_type: if press.event_type == xinput::BUTTON_PRESS_EVENT { xproto::BUTTON_PRESS_EVENT } else { xproto::BUTTON_RELEASE_EVENT },
                    detail: press.detail as u8,
                    sequence: press.sequence,
                    time: press.time,
                    root: press.root,
                    event: press.event,
                    child: press.child,
                    root_x: pixel(press.root_x),
                    root_y: pixel(press.root_y),
                    event_x: pixel(press.event_x),
                    event_y: pixel(press.event_y),
                    state: state.into(),
                    same_screen: true,
                };
                Heard::Core(if press.event_type == xinput::BUTTON_PRESS_EVENT { Event::ButtonPress(core) } else { Event::ButtonRelease(core) })
            }
            Event::XinputEnter(_) => {
                self.forget();
                Heard::Nothing
            }
            Event::XinputDeviceChanged(changed) => {
                self.forget();
                self.devices.insert(changed.sourceid, axes(&changed.classes));
                Heard::Nothing
            }
            other => Heard::Core(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(at: f64) -> xinput::Fp3232 {
        xinput::Fp3232 { integral: at.floor() as i32, frac: ((at - at.floor()) * 4_294_967_296.0) as u32 }
    }

    #[test]
    fn a_scrolling_axis_moved_is_so_many_notches() {
        // A trackpad's: up and down on valuator 3, a notch each 15 along
        // it; across on valuator 2, a notch each -15, turned the other way.
        let mut axes = vec![Axis { number: 3, down: true, notch: 15.0, last: None }, Axis { number: 2, down: false, notch: -15.0, last: None }];
        // Valuators 0, 2 and 3 named: the first time, nowhere yet.
        let mask = [0b1101];
        assert_eq!(scrolled(&mut axes, &mask, &[value(100.0), value(30.0), value(45.0)]), (0.0, 0.0));
        // Down a fifth of a notch, and left half of one.
        let (right, down) = scrolled(&mut axes, &mask, &[value(101.0), value(37.5), value(48.0)]);
        assert!((down - 0.2).abs() < 1e-6 && (right + 0.5).abs() < 1e-6, "{right} {down}");
        // Only valuator 3 named, up three notches; the axis across stays.
        let (right, down) = scrolled(&mut axes, &[0b1000], &[value(3.0)]);
        assert!(right == 0.0 && (down + 3.0).abs() < 1e-6, "{right} {down}");
        // A valuator past the first 32, and none a scrolling axis.
        assert_eq!(scrolled(&mut axes, &[0, 1], &[value(9.0)]), (0.0, 0.0));
        // Many small strokes come to the same zoom as a notch.
        let mut zoom = 1.0;
        for _ in 0..12 {
            zoom *= super::super::wheel_zoom(1.0 / 12.0);
        }
        assert!((zoom - super::super::wheel_zoom(1.0)).abs() < 1e-5);
    }
}

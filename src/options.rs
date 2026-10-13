use std::ffi::OsString;
use std::path::PathBuf;

use crate::canvas::Rect;
use crate::editor::{Finish, Settings, PALETTE};

pub const USAGE: &str = "\
Usage: hotshot [OPTIONS]

Freezes the screen. Drag out an area, draw on it if you like, and copy
or save it. With --full, --monitor, --window or --region there is
nothing to drag out: the picture is taken at once.

What to take:
  -f, --full                 the whole screen, every monitor of it
  -m, --monitor N            one monitor, as --monitors numbers them, or
                             'pointer' for the one the pointer is on
  -w, --window               the window that has the keyboard
  -g, --region X,Y,W,H       this rectangle of the screen (also WxH+X+Y)
  -a, --again                the same area as the last picture was of,
                             however that one was taken
  -e, --edit                 with any of those: open with that area selected,
                             to be drawn on, rather than taking it at once
  -D, --delay SEC            wait before freezing the screen

What to do with it:
  -c, --copy                 copy it to the clipboard
  -s, --save                 save it in the screenshots folder, and print
                             where
  -o, --output FILE          save it as FILE instead; - is standard output
  -d, --dir DIR              the screenshots folder (~/Pictures/Screenshots)
      --filename NAME        what a saved picture is called, less the .png,
                             with the time as strftime sets it out
                             (screenshot-%Y-%m-%d-%H%M%S)
  When dragging an area out, these say what Enter does; the usual is to
  copy. Taken at once, a picture is saved unless told otherwise.

Look:
      --accent COLOR         colour of the selection and of what is picked out:
                             RRGGBB, or one of the Radix colours such as
                             indigo (the default), blue, crimson or teal
      --color COLOR          colour to draw in to begin with: red (the
                             default) or any other of the Radix colours
                             the colour box has, white, black or RRGGBB
      --size N               width of line to begin with, 1 to 4
      --corner-radius N      how round a drawn rectangle's corners are; 0 is
                             square
      --rounded N            round the corners of the picture itself by N
                             pixels, leaving what is outside them see-through
                             (K, or its button, goes through 8, 16 and 28)
      --dim PERCENT          how much darker what is not selected is made
      --magnifier            a magnifier beside the pointer while an edge is
                             being placed (config: magnifier = true)

Other:
      --monitors             list the monitors and quit
      --benchmark            freeze the screen, drag, draw and encode by itself,
                             timing each, print the figures and quit
      --debug                print how long each step of starting up took
                             (also HOTSHOT_DEBUG=1)
      --default-config       print a config file with every setting in it
  -h, --help                 show this help
  -v, --version              show the version

Keys, once an area is selected:
  Enter           copy (or what -c, -s and -o say)     Esc     cancel
  Ctrl+C  copy    Ctrl+S  save    Ctrl+Z  undo    Ctrl+Shift+Z  redo
  V move   A arrow   L line   R rectangle   O oval   P pen   H marker
  T text   N counter   B blur   C colours   I pick a colour off the screen
  F filled   1-4 or the wheel: width of line   Shift: square, circle, 45\u{b0}
  K rounds the corners of the picture, more each time and then not at all
  What is drawn is picked out, with the move tool in hand: drag it, or a
  handle of its box to resize it, or the dot in a rectangle's corner to
  round it. Shift or Ctrl with a click picks out several.
  Arrows nudge the selection, or the shape picked out; Shift by ten;
  Ctrl+arrows resize. Delete removes the shape picked out. Ctrl+A is the
  whole screen. A right click is the colours.

Config file:
  ~/.config/hotshot/config.toml sets defaults, using the long option names:
      dir = \"~/Pictures/shots\"
      enter = \"both\"
      magnifier = true
";

/// What the picture is of.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Take {
    /// An area dragged out by hand.
    Pick,
    Full,
    /// A monitor by its number, or the one the pointer is on.
    Monitor(Option<usize>),
    Window,
    Region(Rect),
    /// The area the last picture was of.
    Again,
}

pub struct Options {
    pub take: Take,
    pub edit: bool,
    pub delay: f64,
    pub copy: bool,
    pub save: bool,
    pub output: Option<PathBuf>,
    pub dir: PathBuf,
    pub filename: String,
    pub settings: Settings,
    pub list_monitors: bool,
    pub benchmark: bool,
    pub debug: bool,
}

pub enum Parsed {
    Run(Box<Options>),
    Help,
    Version,
    Config,
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

/// A path as typed, with a leading `~` taken for the home directory.
fn path(text: &str) -> PathBuf {
    match text.strip_prefix("~/") {
        Some(rest) => home().join(rest),
        None if text == "~" => home(),
        None => PathBuf::from(text),
    }
}

fn defaults() -> Options {
    Options {
        take: Take::Pick,
        edit: false,
        delay: 0.0,
        copy: false,
        save: false,
        output: None,
        dir: home().join("Pictures/Screenshots"),
        filename: "screenshot-%Y-%m-%d-%H%M%S".into(),
        settings: Settings::default(),
        list_monitors: false,
        benchmark: false,
        debug: std::env::var_os("HOTSHOT_DEBUG").is_some_and(|value| !value.is_empty() && value != "0"),
    }
}

/// The long name of each short flag.
fn short_flag(flag: char) -> Option<&'static str> {
    Some(match flag {
        'f' => "full",
        'm' => "monitor",
        'w' => "window",
        'g' => "region",
        'a' => "again",
        'e' => "edit",
        'D' => "delay",
        'c' => "copy",
        's' => "save",
        'o' => "output",
        'd' => "dir",
        'h' => "help",
        'v' => "version",
        _ => return None,
    })
}

fn takes_value(name: &str) -> bool {
    ["monitor", "region", "delay", "output", "dir", "filename", "accent", "color", "size", "corner-radius", "rounded", "dim", "enter"].contains(&name)
}

/// Sets one option, named the way the config file spells it. A flag
/// given no value is switched on.
fn apply(options: &mut Options, name: &str, value: &mut dyn FnMut() -> Result<String, String>) -> Result<(), String> {
    let number = |text: String, what: &str| text.parse::<f64>().ok().filter(|value| value.is_finite() && *value >= 0.0).ok_or(format!("invalid {what} '{text}'"));
    match name {
        "full" => options.take = Take::Full,
        "window" => options.take = Take::Window,
        "monitor" => {
            let text = value()?;
            options.take = Take::Monitor(match text.as_str() {
                "pointer" => None,
                _ => Some(text.parse().map_err(|_| format!("invalid monitor '{text}', expected a number or 'pointer'"))?),
            });
        }
        "region" => options.take = Take::Region(parse_region(&value()?)?),
        "again" => options.take = Take::Again,
        "edit" => options.edit = true,
        "delay" => options.delay = number(value()?, "delay")?,
        "copy" => options.copy = true,
        "save" => options.save = true,
        "output" => options.output = Some(path(&value()?)),
        "dir" => options.dir = path(&value()?),
        "filename" => options.filename = value()?,
        "accent" => options.settings.accent = parse_accent(&value()?)?,
        "color" => options.settings.color = parse_ink(&value()?)?,
        "size" => {
            let text = value()?;
            let size = text.parse::<usize>().ok().filter(|size| (1..=4).contains(size)).ok_or(format!("invalid size '{text}', expected 1 to 4"))?;
            options.settings.size = size - 1;
        }
        "corner-radius" => options.settings.radius = number(value()?, "corner radius")? as f32,
        "rounded" => options.settings.corners = number(value()?, "rounding")? as f32,
        "dim" => options.settings.dim = (number(value()?, "dim")? as u32).min(100),
        "magnifier" => options.settings.magnifier = true,
        "no-magnifier" => options.settings.magnifier = false,
        "enter" => {
            let text = value()?;
            options.settings.enter = match text.as_str() {
                "copy" => Finish { copy: true, save: false },
                "save" => Finish { copy: false, save: true },
                "both" => Finish { copy: true, save: true },
                _ => return Err(format!("invalid enter '{text}', expected copy, save or both")),
            };
        }
        "monitors" => options.list_monitors = true,
        "benchmark" => options.benchmark = true,
        "debug" => options.debug = true,
        _ => return Err(format!("unknown option {name}")),
    }
    Ok(())
}

/// Where the config file is: under `$XDG_CONFIG_HOME`, or `~/.config`.
pub fn config_file() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME").filter(|dir| !dir.is_empty()).map(PathBuf::from).unwrap_or_else(|| home().join(".config"));
    base.join("hotshot/config.toml")
}

/// Reads settings from the text of a config file: a line each, as
/// `name = value`, the value in quotes or not. Something switched off
/// is `name = false`.
fn read_config(options: &mut Options, text: &str) -> Result<(), String> {
    for (number, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let wrong = |error: String| format!("line {}: {error}", number + 1);
        let (name, value) = line.split_once('=').ok_or_else(|| wrong("expected name = value".into()))?;
        let (name, value) = (name.trim(), value.trim());
        // What follows a # is a remark, unless it is inside the quotes.
        let value = match value.strip_prefix('"') {
            Some(quoted) => quoted.split_once('"').map(|(inside, _)| inside).ok_or_else(|| wrong("a quote is not closed".into()))?,
            None => value.split('#').next().unwrap_or("").trim(),
        };
        let name = match (takes_value(name), value) {
            (true, _) => name.to_owned(),
            (false, "true") => name.to_owned(),
            (false, "false") if name == "magnifier" => "no-magnifier".to_owned(),
            (false, "false") => continue,
            (false, _) => return Err(wrong(format!("{name} is either true or false"))),
        };
        apply(options, &name, &mut || Ok(value.to_owned())).map_err(wrong)?;
    }
    Ok(())
}

pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Parsed, String> {
    let mut options = defaults();
    let file = config_file();
    if let Ok(text) = std::fs::read_to_string(&file) {
        read_config(&mut options, &text).map_err(|error| format!("{}: {error}", file.display()))?;
    }

    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let text = arg.into_string().map_err(|_| "an argument is not text".to_string())?;
        // Each entry is a flag name and any value attached to it.
        let mut flags: Vec<(String, Option<String>)> = Vec::new();
        if let Some(long) = text.strip_prefix("--") {
            match long.split_once('=') {
                Some((name, value)) => flags.push((name.into(), Some(value.into()))),
                None => flags.push((long.into(), None)),
            }
        } else if let Some(cluster) = text.strip_prefix('-').filter(|cluster| !cluster.is_empty()) {
            for (at, flag) in cluster.char_indices() {
                let name = short_flag(flag).ok_or_else(|| format!("unknown option -{flag}"))?;
                let rest = &cluster[at + flag.len_utf8()..];
                if takes_value(name) && !rest.is_empty() {
                    flags.push((name.into(), Some(rest.into())));
                    break;
                }
                flags.push((name.into(), None));
            }
        } else {
            return Err(format!("unexpected argument '{text}'"));
        }

        for (name, attached) in flags {
            let mut value = || -> Result<String, String> {
                let next = attached.clone().or_else(|| args.next().and_then(|arg| arg.into_string().ok()));
                next.ok_or_else(|| format!("option --{name} needs a value"))
            };
            match name.as_str() {
                "help" => return Ok(Parsed::Help),
                "version" => return Ok(Parsed::Version),
                "default-config" => return Ok(Parsed::Config),
                name => apply(&mut options, name, &mut value).map_err(|error| error.replace("unknown option ", "unknown option --"))?,
            }
        }
    }
    Ok(Parsed::Run(Box::new(options)))
}

/// A rectangle as `X,Y,W,H` or as `WxH+X+Y`.
fn parse_region(text: &str) -> Result<Rect, String> {
    let wrong = || format!("invalid region '{text}', expected X,Y,W,H or WxH+X+Y");
    let numbers = |parts: Vec<&str>| -> Option<Vec<i32>> { parts.iter().map(|part| part.trim().parse().ok()).collect() };
    let rect = if text.contains('x') {
        let parts = numbers(text.split(['x', '+']).collect()).ok_or_else(wrong)?;
        match parts[..] {
            [w, h, x, y] => Rect::new(x, y, w, h),
            _ => return Err(wrong()),
        }
    } else {
        let parts = numbers(text.split(',').collect()).ok_or_else(wrong)?;
        match parts[..] {
            [x, y, w, h] => Rect::new(x, y, w, h),
            _ => return Err(wrong()),
        }
    };
    if rect.is_empty() { Err(wrong()) } else { Ok(rect) }
}

fn parse_color(text: &str) -> Option<u32> {
    let hex = text.trim_start_matches('#');
    u32::from_str_radix(hex, 16).ok().filter(|_| hex.len() == 6)
}

/// A colour to draw in: one of the palette's by name, or `RRGGBB`.
fn parse_ink(text: &str) -> Result<u32, String> {
    let named = PALETTE.iter().find(|(_, name)| *name == text.to_lowercase()).map(|(color, _)| *color);
    named.or_else(|| parse_color(text)).ok_or(format!("invalid colour '{text}', expected a name such as red, or RRGGBB"))
}

/// An accent colour: one of the Radix colours by name, at the step meant
/// for solid backgrounds, or any colour as `RRGGBB`. The same as jog's.
fn parse_accent(text: &str) -> Result<u32, String> {
    let named = match text.to_lowercase().as_str() {
        "tomato" => 0xE54D2E,
        "red" => 0xE5484D,
        "ruby" => 0xE54666,
        "crimson" => 0xE93D82,
        "pink" => 0xD6409F,
        "plum" => 0xAB4ABA,
        "purple" => 0x8E4EC6,
        "violet" => 0x6E56CF,
        "iris" => 0x5B5BD6,
        "indigo" => 0x3E63DD,
        "blue" => 0x0090FF,
        "cyan" => 0x00A2C7,
        "teal" => 0x12A594,
        "jade" => 0x29A383,
        "green" => 0x30A46C,
        "grass" => 0x46A758,
        "orange" => 0xF76B15,
        "brown" => 0xAD7F58,
        "bronze" => 0xA18072,
        "gold" => 0x978365,
        _ => return parse_color(text).ok_or(format!("invalid accent '{text}', expected a Radix colour name such as indigo, or RRGGBB")),
    };
    Ok(named)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(args: &[&str]) -> Result<Options, String> {
        match parse(args.iter().map(OsString::from))? {
            Parsed::Run(options) => Ok(*options),
            _ => Err("not a run".into()),
        }
    }

    #[test]
    fn flags_cluster_and_take_values_either_way() {
        let options = run(&["-fc", "-o", "out.png", "--delay=2", "-D0.5"]).unwrap();
        assert_eq!(options.take, Take::Full);
        assert!(options.copy && !options.save);
        assert_eq!(options.output, Some(PathBuf::from("out.png")));
        assert_eq!(options.delay, 0.5);
        assert!(run(&["--nonsense"]).err().unwrap().contains("unknown option --nonsense"));
        assert!(run(&["-o"]).err().unwrap().contains("needs a value"));
        assert!(run(&["stray"]).is_err());
    }

    #[test]
    fn regions_are_read_both_ways() {
        assert_eq!(run(&["-g", "10,20,300,400"]).unwrap().take, Take::Region(Rect::new(10, 20, 300, 400)));
        assert_eq!(run(&["--region", "300x400+10+20"]).unwrap().take, Take::Region(Rect::new(10, 20, 300, 400)));
        assert!(run(&["-g", "10,20,0,400"]).is_err());
        assert_eq!(run(&["-m", "pointer"]).unwrap().take, Take::Monitor(None));
        assert_eq!(run(&["-m1"]).unwrap().take, Take::Monitor(Some(1)));
    }

    #[test]
    fn a_config_file_sets_what_the_flags_do() {
        let mut options = defaults();
        assert!(!options.settings.magnifier);
        let text = "# a remark\ndir = \"/tmp/shots\"   # and another\nenter = \"both\"\nmagnifier = true\nsize = 3\naccent = \"teal\"\ncolor = \"00ff00\"\nrounded = 16\n";
        read_config(&mut options, text).unwrap();
        assert!(options.settings.magnifier && options.settings.corners == 16.0);
        read_config(&mut options, "magnifier = false").unwrap();
        assert_eq!(options.dir, PathBuf::from("/tmp/shots"));
        assert_eq!(options.settings.enter, Finish { copy: true, save: true });
        assert!(!options.settings.magnifier);
        assert_eq!((options.settings.size, options.settings.accent, options.settings.color), (2, 0x12A594, 0x00FF00));
        read_config(&mut options, "color = \"jade\"").unwrap();
        assert_eq!(options.settings.color, 0x29A383);
        assert!(read_config(&mut options, "size = 9").unwrap_err().starts_with("line 1"));
        assert!(read_config(&mut options, "nothing").is_err());
    }
}

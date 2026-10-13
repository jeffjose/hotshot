//! Reading the frozen screen a strip at a time. Out of some graphics
//! cards a 4K screen is a quarter of a second coming, however it is
//! asked for; taken in strips, with the pointer and the keys seen to
//! between one strip and the next, that quarter of a second is not
//! spent deaf. The strips nearest the pointer come first, since that
//! is where whatever is done first will be done.
//!
//! Until a strip has come, nothing is drawn in it: what shows there is
//! the screen as the server darkened it, which is right until something
//! is selected or put over it, and is put right as soon as the strip is
//! in.

use std::ops::Range;

use x11rb::protocol::shm;

use crate::canvas::{self, Rect};
use crate::editor::Editor;
use crate::x11::{Display, Result};

/// How many strips the screen is read in: enough that none is long in
/// coming, and few enough that asking for them is no work.
const STRIPS: i32 = 32;

pub struct Strips {
    segment: shm::Seg,
    width: i32,
    height: i32,
    /// How many rows a strip is.
    rows: i32,
    /// The strips still to come, the next of them last.
    coming: Vec<i32>,
    /// Whether each has come.
    have: Vec<bool>,
}

impl Strips {
    /// The strips of a screen, to be read into shared memory in order of
    /// how near they are to a row of it.
    pub fn new(segment: shm::Seg, width: i32, height: i32, near: i32) -> Strips {
        let rows = (height + STRIPS - 1) / STRIPS;
        let count = (height + rows - 1) / rows.max(1);
        let mut coming: Vec<i32> = (0..count).collect();
        coming.sort_by_key(|strip| std::cmp::Reverse((strip * rows + rows / 2 - near).abs()));
        Strips { segment, width, height, rows, coming, have: vec![false; count as usize] }
    }

    /// For a screen that was read all at once: nothing is still to come.
    pub fn have_all(&mut self) {
        self.coming.clear();
        self.have.fill(true);
    }

    pub fn done(&self) -> bool {
        self.coming.is_empty()
    }

    /// Reads the next strip, has the editor take it in, and puts it,
    /// darkened, in the frame the display is taken to be showing.
    pub fn read(&mut self, display: &Display, editor: &mut Editor, frame: &mut [u32]) -> Result<()> {
        let Some(strip) = self.coming.pop() else { return Ok(()) };
        let rows = strip * self.rows..((strip + 1) * self.rows).min(self.height);
        display.read_rows(self.segment, rows.clone())?;
        editor.arrived(rows.clone());
        let pixels = (rows.start * self.width) as usize..(rows.end * self.width) as usize;
        frame[pixels.clone()].copy_from_slice(&editor.dimmed()[pixels]);
        self.have[strip as usize] = true;
        Ok(())
    }

    /// The rows that have come, as the runs of them that touch.
    fn runs(&self) -> Vec<Range<i32>> {
        let mut runs: Vec<Range<i32>> = Vec::new();
        for strip in (0..self.have.len() as i32).filter(|&strip| self.have[strip as usize]) {
            let rows = strip * self.rows..((strip + 1) * self.rows).min(self.height);
            match runs.last_mut() {
                Some(last) if last.end == rows.start => last.end = rows.end,
                _ => runs.push(rows),
            }
        }
        runs
    }

    /// Where a new frame differs from the one showing, in the rows that
    /// have come; the rest of it is not looked at, and not shown.
    pub fn changed(&self, now: &[u32], was: &mut [u32]) -> Vec<Rect> {
        let width = self.width as usize;
        let mut parts = Vec::new();
        for rows in self.runs() {
            let pixels = rows.start as usize * width..rows.end as usize * width;
            let found = canvas::changed(&now[pixels.clone()], &mut was[pixels], width);
            parts.extend(found.into_iter().map(|part| part.moved(0, rows.start)));
        }
        parts
    }

    /// The rows that have come, as rectangles of the screen.
    pub fn had(&self) -> Vec<Rect> {
        self.runs().into_iter().map(|rows| Rect::new(0, rows.start, self.width, rows.end - rows.start)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_strips_nearest_come_first_and_cover_the_screen() {
        let strips = Strips::new(0, 100, 1000, 510);
        assert_eq!(strips.rows, 32);
        assert_eq!(strips.coming.len(), 32);
        // The last of the list is the next to be read.
        assert_eq!(strips.coming[strips.coming.len() - 1], 15);
        let mut seen: Vec<i32> = strips.coming.clone();
        seen.sort_unstable();
        assert_eq!(seen, (0..32).collect::<Vec<i32>>());
        assert!(strips.runs().is_empty());
    }

    #[test]
    fn only_the_rows_that_have_come_are_compared() {
        let mut strips = Strips::new(0, 10, 64, 0);
        strips.have[1] = true;
        assert_eq!(strips.had(), [Rect::new(0, 2, 10, 2)]);
        let now = vec![1u32; 10 * 64];
        let mut was = vec![0u32; 10 * 64];
        assert_eq!(strips.changed(&now, &mut was), [Rect::new(0, 2, 10, 2)]);
        // What was compared is brought up to date, and nothing else.
        assert_eq!(was.iter().filter(|&&pixel| pixel == 1).count(), 20);
        strips.have[2] = true;
        assert_eq!(strips.had(), [Rect::new(0, 2, 10, 4)]);
    }
}

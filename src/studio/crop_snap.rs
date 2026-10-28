//! A crop kept to the edges found in its picture: its sides snapped to
//! them as they are pulled, made tight to what is in the picture, and
//! fitted round the box of them double clicked in.

use super::edges::Lines;
use super::*;

impl Studio {
    /// The lines of a picture: found the first time it is cropped, and
    /// kept for as long as it is the one cropped.
    pub(super) fn crop_lines(&mut self, image: &Arc<Image>) -> Arc<Lines> {
        if let Some((known, lines)) = &self.crop_edges {
            if std::ptr::eq(known.as_ptr(), Arc::as_ptr(image)) {
                return lines.clone();
            }
        }
        let began = std::time::Instant::now();
        let (w, h, pixels) = &image.levels[0];
        let lines = Arc::new(Lines::find(*w, *h, pixels));
        debug!("canvas found {} lines down and {} across a picture {w} by {h} in {:.1?}", lines.upright.len(), lines.across.len(), began.elapsed());
        self.crop_edges = Some((Arc::downgrade(image), lines.clone()));
        lines
    }
}

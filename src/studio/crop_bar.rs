//! The bar floating over a picture being cropped.

use super::*;

impl Studio {
    /// Over the picture being cropped, the shapes the crop can keep to and
    /// a reset, floating.
    pub(super) fn layout_crop_bar(&mut self, layout: &mut Layout) {
        let Some(Layer { kind: Kind::Picture(picture), .. }) = self.cropping.and_then(|index| self.doc.layers.get(index)).cloned() else { return };
        let top = picture.corners().map(|corner| self.to_window(corner));
        let (left, right, high) = (top.iter().map(|c| c.0).min().unwrap_or(0), top.iter().map(|c| c.0).max().unwrap_or(0), top.iter().map(|c| c.1).min().unwrap_or(0));
        let mut items: Vec<(String, Control)> = panel::CROP_ASPECTS.iter().enumerate().map(|(at, (name, _))| (name.to_string(), Control::Action(Action::Aspect(at as u8)))).collect();
        items.push(("Reset".into(), Control::Action(Action::ResetCrop)));
        // Wider than the square buttons of the panel, which a narrow one,
        // as 1:1 is, would otherwise be drawn as.
        let wides: Vec<i32> = items.iter().map(|(says, _)| (self.text.width(says, 12) + 16).max(34)).collect();
        let wide = wides.iter().sum::<i32>() + 4 * (wides.len() as i32 - 1) + 12;
        let room = self.open_area();
        let x = ((left + right) / 2 - wide / 2).clamp(room.x + 4, (room.right() - wide - 4).max(room.x));
        let y = (high - 52).max(room.y + 4);
        let bar = Rect::new(x, y, wide, 34);
        let mut at = x + 6;
        let line = Text::line_height(12);
        for ((says, control), w) in items.into_iter().zip(wides) {
            layout.controls.insert(0, (Rect::new(at, y + 5, w, 24), control));
            let inset = (w - self.text.width(&says, 12)) / 2;
            layout.words.push(((at + inset, y + 5 + (24 - line) / 2), says, TEXT_COLOR, 12));
            at += w + 4;
        }
        layout.controls.push((bar, Control::Panel));
        layout.crop_bar = Some(bar);
    }
}

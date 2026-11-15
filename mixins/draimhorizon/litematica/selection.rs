// A port of Litematica's area selection (LGPL-3.0, see NOTICE.md): `Box` (two corners and the size
// the pair makes, signed like the original's), `AreaSelection` (named boxes, the one being edited,
// an origin that is the first box's corner unless set) and the unique naming of a new box
// (`createNewSubRegionBox`). Pure.
pub type Pos = [i32; 3];

/// Which corner of a box is meant (`CornerSelectionMode` leaves out the third, "none").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Corner {
    One,
    Two,
}

/// `Box`: a name and two corners, either of which may be unset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Box {
    pub name: String,
    pub pos1: Option<Pos>,
    pub pos2: Option<Pos>,
}
impl Box {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            pos1: None,
            pos2: None,
        }
    }
    pub fn set(&mut self, corner: Corner, pos: Pos) {
        match corner {
            Corner::One => self.pos1 = Some(pos),
            Corner::Two => self.pos2 = Some(pos),
        }
    }
    /// `updateSize` / `getSize`: both corners set, the size runs from the first to the second and
    /// is negative along an axis the second corner lies behind the first on; one corner alone is
    /// a single block; none is nothing.
    pub fn size(&self) -> Pos {
        match (self.pos1, self.pos2) {
            (Some(a), Some(b)) => [0, 1, 2].map(|i| {
                let d = b[i] - a[i];
                if d >= 0 { d + 1 } else { d - 1 }
            }),
            (None, None) => [0; 3],
            _ => [1; 3],
        }
    }
    /// Both corners are set.
    pub fn complete(&self) -> bool {
        self.pos1.is_some() && self.pos2.is_some()
    }
    /// The smallest and the largest block of a complete box.
    pub fn bounds(&self) -> Option<(Pos, Pos)> {
        let (a, b) = (self.pos1?, self.pos2?);
        Some((
            [0, 1, 2].map(|i| a[i].min(b[i])),
            [0, 1, 2].map(|i| a[i].max(b[i])),
        ))
    }
    /// How many blocks the box holds.
    pub fn volume(&self) -> u64 {
        self.size()
            .iter()
            .map(|v| u64::from(v.unsigned_abs()))
            .product()
    }
    /// The box moved by `by`.
    pub fn moved(&self, by: Pos) -> Self {
        let add = |p: Pos| [0, 1, 2].map(|i| p[i].saturating_add(by[i]));
        Self {
            name: self.name.clone(),
            pos1: self.pos1.map(add),
            pos2: self.pos2.map(add),
        }
    }
}

/// `AreaSelection`: the boxes in the order they were made, the one being edited, the origin.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    boxes: Vec<Box>,
    current: Option<usize>,
    explicit_origin: Option<Pos>,
}
/// The most boxes a selection holds (a Litematica file holds at most this many regions).
pub const MAX_BOXES: usize = 16;

impl Selection {
    pub fn boxes(&self) -> &[Box] {
        &self.boxes
    }
    pub fn is_empty(&self) -> bool {
        self.boxes.is_empty()
    }
    /// `getSelectedSubRegionBox`.
    pub fn current(&self) -> Option<&Box> {
        self.current.and_then(|i| self.boxes.get(i))
    }
    /// `createNewSubRegionBox`: a box with both corners at `pos`, named `name` (`name 1`, `name 2`
    /// ... when it is taken), and made the current one. `None` at the limit.
    pub fn new_box(&mut self, pos: Pos, name: &str) -> Option<String> {
        if self.boxes.len() >= MAX_BOXES {
            return None;
        }
        let mut unique = name.to_owned();
        let mut i = 1;
        while self.boxes.iter().any(|b| b.name == unique) {
            unique = format!("{name} {i}");
            i += 1;
        }
        let mut b = Box::new(&unique);
        b.set(Corner::One, pos);
        b.set(Corner::Two, pos);
        self.boxes.push(b);
        self.current = Some(self.boxes.len() - 1);
        Some(unique)
    }
    /// A corner of the current box (a first box is made when there is none).
    pub fn set_corner(&mut self, corner: Corner, pos: Pos) {
        match self.current {
            Some(i) if i < self.boxes.len() => self.boxes[i].set(corner, pos),
            _ => {
                self.new_box(pos, "Box");
            }
        }
    }
    /// `removeSelectedSubRegionBox`.
    pub fn remove_current(&mut self) -> bool {
        match self.current.take() {
            Some(i) if i < self.boxes.len() => {
                self.boxes.remove(i);
                self.current = self.boxes.len().checked_sub(1);
                true
            }
            _ => false,
        }
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    /// `setExplicitOrigin`.
    pub fn set_origin(&mut self, origin: Option<Pos>) {
        self.explicit_origin = origin;
    }
    /// `getEffectiveOrigin`: the set origin, or the first corner of the first box.
    pub fn origin(&self) -> Option<Pos> {
        self.explicit_origin
            .or_else(|| self.boxes.first().and_then(|b| b.pos1))
    }
    /// `moveEntireSelectionTo`: everything moves so that the origin lands on `to`.
    pub fn move_to(&mut self, to: Pos) {
        let Some(from) = self.origin() else {
            return;
        };
        let by = [0, 1, 2].map(|i| to[i].saturating_sub(from[i]));
        self.boxes = self.boxes.iter().map(|b| b.moved(by)).collect();
        if let Some(o) = self.explicit_origin.as_mut() {
            *o = to;
        }
    }
    /// The smallest and the largest block over every complete box.
    pub fn enclosing(&self) -> Option<(Pos, Pos)> {
        let mut out: Option<(Pos, Pos)> = None;
        for (lo, hi) in self.boxes.iter().filter_map(Box::bounds) {
            out = Some(match out {
                None => (lo, hi),
                Some((a, b)) => (
                    [0, 1, 2].map(|i| a[i].min(lo[i])),
                    [0, 1, 2].map(|i| b[i].max(hi[i])),
                ),
            });
        }
        out
    }
    /// Whether the selection can be saved: every box complete, and no more cells than `max`.
    pub fn check(&self, max_volume: u64) -> Result<(), &'static str> {
        if self.boxes.is_empty() {
            return Err("empty");
        }
        if self.boxes.iter().any(|b| !b.complete()) {
            return Err("incomplete");
        }
        if self.boxes.iter().map(Box::volume).sum::<u64>() > max_volume {
            return Err("too large");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_box_size_runs_from_the_first_corner_to_the_second_and_is_signed() {
        let mut b = Box::new("a");
        assert_eq!(b.size(), [0; 3]);
        b.set(Corner::One, [10, 64, 10]);
        assert_eq!(b.size(), [1; 3]);
        b.set(Corner::Two, [12, 60, 10]);
        assert_eq!(b.size(), [3, -5, 1]);
        assert_eq!(b.volume(), 15);
        assert_eq!(b.bounds(), Some(([10, 60, 10], [12, 64, 10])));
        assert_eq!(b.moved([1, 1, 1]).pos1, Some([11, 65, 11]));
    }

    #[test]
    fn new_boxes_get_unique_names_and_the_origin_is_the_first_corner() {
        let mut s = Selection::default();
        assert_eq!(s.origin(), None);
        assert_eq!(s.new_box([5, 5, 5], "Box").as_deref(), Some("Box"));
        assert_eq!(s.new_box([9, 5, 5], "Box").as_deref(), Some("Box 1"));
        assert_eq!(s.new_box([1, 1, 1], "Box").as_deref(), Some("Box 2"));
        assert_eq!(s.origin(), Some([5, 5, 5]));
        assert_eq!(s.current().map(|b| b.name.as_str()), Some("Box 2"));
        s.set_corner(Corner::Two, [3, 3, 3]);
        assert_eq!(s.current().unwrap().size(), [3, 3, 3]);
        assert_eq!(s.enclosing(), Some(([1, 1, 1], [9, 5, 5])));
        assert!(s.remove_current());
        assert_eq!(s.boxes().len(), 2);
        s.set_origin(Some([0, 0, 0]));
        assert_eq!(s.origin(), Some([0, 0, 0]));
        s.move_to([10, 0, 0]);
        assert_eq!(s.boxes()[0].pos1, Some([15, 5, 5]));
        assert_eq!(s.origin(), Some([10, 0, 0]));
    }

    #[test]
    fn a_selection_is_checked_before_it_is_saved() {
        let mut s = Selection::default();
        assert_eq!(s.check(100), Err("empty"));
        s.set_corner(Corner::One, [0, 0, 0]);
        assert_eq!(s.check(100), Ok(()));
        s.set_corner(Corner::Two, [9, 9, 9]);
        assert_eq!(s.check(100), Err("too large"));
        assert_eq!(s.check(1000), Ok(()));
        for i in 0..MAX_BOXES {
            let _ = s.new_box([i as i32; 3], "b");
        }
        assert!(s.new_box([0; 3], "b").is_none());
        s.clear();
        assert!(s.is_empty());
    }
}

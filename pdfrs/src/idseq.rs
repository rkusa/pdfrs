/// A type used retrieve sequential ids.
pub struct IdSeq {
    next_id: usize,
}

impl IdSeq {
    /// Constructs a new `IdSeq` starting with `next_id` as the next ID in its sequence.
    pub fn new(next_id: usize) -> Self {
        IdSeq { next_id }
    }

    /// Retrieves the next id.
    pub fn next(&mut self) -> usize {
        let next = self.next_id;
        self.next_id += 1;
        next
    }

    /// Returns the amount of IDs that have been handed-out.
    ///
    /// The `count` is always relative to a sequence start of `1`, regardless of whether the
    /// sequence was initiated with a higher `next_id`.
    pub fn count(&mut self) -> usize {
        self.next_id - 1
    }
}

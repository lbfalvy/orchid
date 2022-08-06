use std::fmt::Display;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {Left, Right}

impl Display for Side {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Left => write!(f, "Left"),
            Self::Right => write!(f, "Right"),
        }
    }
}

impl Side {
    pub fn opposite(&self) -> Self {
        match self {
            Self::Left => Self::Right, 
            Self::Right => Self::Left
        }
    }
    /// Shorthand for opposite
    pub fn inv(&self) -> Self { self.opposite() }
    /// take N elements from this end of a slice
    pub fn slice<'a, T>(&self, size: usize, slice: &'a [T]) -> &'a [T] {
        match self {
            Side::Left => &slice[..size],
            Side::Right => &slice[slice.len() - size..]
        }
    }
    /// ignore N elements from this end of a slice
    pub fn crop<'a, T>(&self, margin: usize, slice: &'a [T]) -> &'a [T] {
        self.opposite().slice(slice.len() - margin, slice)
    }
    /// ignore N elements from this end and M elements from the other end of a slice
    pub fn crop_both<'a, T>(&self, margin: usize, opposite: usize, slice: &'a [T]) -> &'a [T] {
        self.crop(margin, self.opposite().crop(opposite, slice))
    }
    /// Pick this side from a pair of things
    pub fn pick<T>(&self, pair: (T, T)) -> T {
        match self {
            Side::Left => pair.0,
            Side::Right => pair.1
        }
    }
    /// Make a pair with the first element on this side
    pub fn pair<T>(&self, this: T, opposite: T) -> (T, T) {
        match self {
            Side::Left => (this, opposite),
            Side::Right => (opposite, this)
        }
    }
}
//! Byte ranges into the source text, and their translation to lines and columns.

/// A half-open byte range `start..end` into the source.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Span { start, end }
    }

    /// The smallest span covering both `self` and `other`.
    pub fn to(self, other: Span) -> Span {
        Span::new(self.start.min(other.start), self.end.max(other.end))
    }
}

/// A 1-based line and column (columns count characters, not bytes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineCol {
    pub line: usize,
    pub col: usize,
}

pub fn line_col(src: &str, offset: usize) -> LineCol {
    let offset = offset.min(src.len());
    let before = &src[..offset];
    let line = before.matches('\n').count() + 1;
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    LineCol { line, col: src[line_start..offset].chars().count() + 1 }
}

/// The text of a 1-based line, without its line break.
pub fn line_text(src: &str, line: usize) -> &str {
    src.lines().nth(line.saturating_sub(1)).unwrap_or("")
}

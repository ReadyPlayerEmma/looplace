//! Platform detection helpers.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Web,
    Desktop,
    Mobile,
    Unknown,
}

impl Platform {
    pub fn current() -> Self {
        // Placeholder until platform-specific glue arrives.
        Self::Unknown
    }
}

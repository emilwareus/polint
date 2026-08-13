//! Opaque registry-assigned language ids.
//!
//! Sorting and display must use frontend profile names, never raw [`LanguageId`] order.

/// Opaque registry-assigned language id.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct LanguageId(u16);

impl LanguageId {
    /// Sentinel for public languages with no registered frontend (e.g. Unknown).
    pub const UNREGISTERED: Self = Self(u16::MAX);
    /// Stable id matching default registration order for Go.
    pub const GO: Self = Self(0);
    /// Stable id matching default registration order for TypeScript/JavaScript.
    pub const TS: Self = Self(1);

    /// Construct from a raw registry ordinal (host composition / tests).
    pub const fn from_raw(raw: u16) -> Self {
        Self(raw)
    }

    /// Borrow the raw registry ordinal.
    pub const fn raw(self) -> u16 {
        self.0
    }
}

pub const LANGUAGE_IDS_NONE: &[LanguageId] = &[];
pub const LANGUAGE_IDS_GO: &[LanguageId] = &[LanguageId::GO];
pub const LANGUAGE_IDS_TS: &[LanguageId] = &[LanguageId::TS];
pub const LANGUAGE_IDS_GO_AND_TS: &[LanguageId] = &[LanguageId::GO, LanguageId::TS];

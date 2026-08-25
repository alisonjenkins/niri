#![deny(clippy::unwrap_used, clippy::expect_used)]

//! Errors from creating and removing virtual outputs.
//!
//! `unwrap` is denied here rather than crate-wide: upstream niri uses it
//! deliberately in plenty of places, and turning it off everywhere would be a
//! change to their code, not ours. The virtual output work is what has been
//! panicking the compositor in practice — twice from unwrapping state a
//! virtual output does not carry — so the rule is applied where it earns its
//! keep.
//!
//! A dedicated type rather than `String`: these cross the IPC boundary, and a
//! caller that wants to tell "that name is already taken" from "this backend
//! cannot do virtual outputs at all" should not have to match on prose. The
//! `Display` text is what reaches the client, so it stays phrased for a person
//! reading `niri msg` output.

use std::fmt;

/// Why a virtual output could not be created or removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VirtualOutputError {
    /// An output already exists under this name.
    ///
    /// Names are how anything finds a virtual output again, so reusing one
    /// silently would hand the caller an output it did not create.
    NameTaken(String),

    /// No virtual output by this name.
    NotFound(String),

    /// The running backend has no virtual output support.
    ///
    /// Winit is nested inside another compositor, which owns the outputs.
    UnsupportedBackend(&'static str),

    /// The shared IPC output map was left poisoned by a panicking thread.
    ///
    /// Reported rather than propagated as a panic: losing the ability to add
    /// an output is a great deal better than taking the session down with it.
    IpcOutputsPoisoned,
}

impl fmt::Display for VirtualOutputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VirtualOutputError::NameTaken(name) => {
                write!(f, "output \"{name}\" already exists")
            }
            VirtualOutputError::NotFound(name) => {
                write!(f, "virtual output \"{name}\" not found")
            }
            VirtualOutputError::UnsupportedBackend(backend) => {
                write!(
                    f,
                    "virtual outputs are not supported with the {backend} backend"
                )
            }
            VirtualOutputError::IpcOutputsPoisoned => {
                write!(
                    f,
                    "the output list is in an inconsistent state; restart niri"
                )
            }
        }
    }
}

impl std::error::Error for VirtualOutputError {}

/// Result of an operation on a virtual output.
pub type VirtualOutputResult<T> = Result<T, VirtualOutputError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_name_the_output_they_are_about() {
        // These reach the user through `niri msg`, so the name has to survive
        // into the text rather than being flattened to a generic failure.
        assert_eq!(
            VirtualOutputError::NameTaken("steam".to_owned()).to_string(),
            "output \"steam\" already exists"
        );
        assert_eq!(
            VirtualOutputError::NotFound("steam".to_owned()).to_string(),
            "virtual output \"steam\" not found"
        );
    }

    #[test]
    fn variants_are_distinguishable_without_reading_prose() {
        // The reason for the type: a caller can branch on the cause rather
        // than matching on message text that is free to change.
        let taken = VirtualOutputError::NameTaken("steam".to_owned());
        let missing = VirtualOutputError::NotFound("steam".to_owned());
        assert_ne!(taken, missing);
        assert!(matches!(taken, VirtualOutputError::NameTaken(_)));
    }
}

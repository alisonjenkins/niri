#![deny(clippy::unwrap_used, clippy::expect_used)]

//! Errors from creating and removing virtual outputs, and the shared bits of
//! how a virtual output presents itself to clients.
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

use niri_config::OutputName;
use smithay::output::Output;
use smithay::utils::{Raw, Size};

/// `OutputName::make` reported by every virtual output, on every backend.
///
/// The single source of truth for the marker a virtual output carries, so
/// spotting one later (see `is_virtual_output`) does not depend on two
/// backends agreeing on a literal by coincidence.
pub const VIRTUAL_OUTPUT_MAKE: &str = "niri";

/// `OutputName::model` reported by every virtual output, on every backend.
pub const VIRTUAL_OUTPUT_MODEL: &str = "virtual";

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

    /// The named output exists but is not a virtual output.
    NotVirtual(String),

    /// The virtual output exists but is turned off.
    Disabled(String),

    /// There is no physical output to show the named virtual output on.
    NoViewer(String),
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
            VirtualOutputError::NotVirtual(name) => {
                write!(f, "output \"{name}\" is not a virtual output")
            }
            VirtualOutputError::Disabled(name) => {
                write!(f, "virtual output \"{name}\" is off")
            }
            VirtualOutputError::NoViewer(name) => {
                write!(f, "no physical output to view \"{name}\" on")
            }
        }
    }
}

impl std::error::Error for VirtualOutputError {}

/// Result of an operation on a virtual output.
pub type VirtualOutputResult<T> = Result<T, VirtualOutputError>;

/// Synthesize a physical size (in millimetres) for a virtual output, given
/// its pixel dimensions.
///
/// A virtual output has no panel, so `0mm x 0mm` is the technically honest
/// answer — and it is also the wrong one. Clients are free to treat a `0mm`
/// physical size as "not a real display" and drop the output entirely; Steam
/// does exactly that, and a virtual output created for Remote Play streaming
/// simply never showed up in its display list. That broke streaming for
/// weeks before anyone traced it back to this. Do not "simplify" this back
/// to `(0, 0)`.
///
/// There is no physically correct size to report instead, so this picks a
/// plausible one: a nominal 96 DPI, the traditional desktop baseline, i.e.
/// `mm = px * 25.4 / 96`. Kept in integer math as `px * 254 / 960` to avoid
/// pulling in floating point for a value nothing downstream expects to be
/// exact.
pub fn physical_size_mm(width: i32, height: i32) -> Size<i32, Raw> {
    let mm_from_px = |px: i32| px.saturating_mul(254) / 960;
    (mm_from_px(width), mm_from_px(height)).into()
}

/// Whether `output` is one of ours, created by [`Tty::create_virtual_output`]
/// or [`Headless::create_virtual_output`], rather than a physical display.
///
/// Both backends stamp every virtual output with the same `OutputName` make
/// and model, so recognising one later is just checking for that marker.
///
/// [`Tty::create_virtual_output`]: super::tty::Tty::create_virtual_output
/// [`Headless::create_virtual_output`]: super::headless::Headless::create_virtual_output
pub fn is_virtual_output(output: &Output) -> bool {
    let Some(name) = output.user_data().get::<OutputName>() else {
        return false;
    };
    name.make.as_deref() == Some(VIRTUAL_OUTPUT_MAKE)
        && name.model.as_deref() == Some(VIRTUAL_OUTPUT_MODEL)
}

#[cfg(test)]
mod tests {
    use niri_config::OutputName;
    use smithay::output::{Output, PhysicalProperties, Subpixel};

    use super::*;

    fn output_with_name(name: Option<OutputName>) -> Output {
        let output = Output::new(
            "steam".to_owned(),
            PhysicalProperties {
                size: (0, 0).into(),
                subpixel: Subpixel::Unknown,
                make: "niri".to_owned(),
                model: "virtual".to_owned(),
                serial_number: "1".to_owned(),
            },
        );
        if let Some(name) = name {
            output.user_data().insert_if_missing(|| name);
        }
        output
    }

    #[test]
    fn is_virtual_output_true_for_niri_virtual_marker() {
        let output = output_with_name(Some(OutputName {
            connector: "steam".to_owned(),
            make: Some(VIRTUAL_OUTPUT_MAKE.to_owned()),
            model: Some(VIRTUAL_OUTPUT_MODEL.to_owned()),
            serial: Some("1".to_owned()),
        }));
        assert!(is_virtual_output(&output));
    }

    #[test]
    fn is_virtual_output_false_when_make_differs() {
        let output = output_with_name(Some(OutputName {
            connector: "DP-1".to_owned(),
            make: Some("Dell Inc.".to_owned()),
            model: Some(VIRTUAL_OUTPUT_MODEL.to_owned()),
            serial: Some("1".to_owned()),
        }));
        assert!(!is_virtual_output(&output));
    }

    #[test]
    fn is_virtual_output_false_when_model_differs() {
        let output = output_with_name(Some(OutputName {
            connector: "DP-1".to_owned(),
            make: Some(VIRTUAL_OUTPUT_MAKE.to_owned()),
            model: Some("U2720Q".to_owned()),
            serial: Some("1".to_owned()),
        }));
        assert!(!is_virtual_output(&output));
    }

    #[test]
    fn is_virtual_output_false_without_output_name() {
        let output = output_with_name(None);
        assert!(!is_virtual_output(&output));
    }

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
        assert_eq!(
            VirtualOutputError::NotVirtual("steam".to_owned()).to_string(),
            "output \"steam\" is not a virtual output"
        );
        assert_eq!(
            VirtualOutputError::Disabled("steam".to_owned()).to_string(),
            "virtual output \"steam\" is off"
        );
        assert_eq!(
            VirtualOutputError::NoViewer("steam".to_owned()).to_string(),
            "no physical output to view \"steam\" on"
        );
    }

    #[test]
    fn variants_are_distinguishable_without_reading_prose() {
        // The reason for the type: a caller can branch on the cause rather
        // than matching on message text that is free to change.
        let taken = VirtualOutputError::NameTaken("steam".to_owned());
        let missing = VirtualOutputError::NotFound("steam".to_owned());
        let not_virtual = VirtualOutputError::NotVirtual("steam".to_owned());
        let disabled = VirtualOutputError::Disabled("steam".to_owned());
        assert_ne!(taken, missing);
        assert_ne!(not_virtual, disabled);
        assert_ne!(disabled, VirtualOutputError::NoViewer("steam".to_owned()));
        assert!(matches!(taken, VirtualOutputError::NameTaken(_)));
        assert!(matches!(not_virtual, VirtualOutputError::NotVirtual(_)));
    }

    #[test]
    fn a_typical_handheld_mode_gets_a_nonzero_size() {
        // The whole point: Steam discards an output reporting 0mm x 0mm, and
        // this is the exact mode that broke Remote Play streaming.
        let size = physical_size_mm(1280, 800);
        assert_eq!((size.w, size.h), (338, 211));
        assert_ne!(size.w, 0);
        assert_ne!(size.h, 0);
    }

    #[test]
    fn a_desktop_sized_mode_gets_a_nonzero_size() {
        let size = physical_size_mm(1920, 1080);
        assert_eq!((size.w, size.h), (508, 285));
    }

    #[test]
    fn zero_pixels_stays_zero_millimetres() {
        // There is no plausible nonzero size for a mode with no pixels; this
        // just documents that the conversion does not divide by zero or
        // otherwise misbehave at the degenerate input.
        let size = physical_size_mm(0, 0);
        assert_eq!((size.w, size.h), (0, 0));
    }

    #[test]
    fn conversion_is_roughly_ninety_six_dpi() {
        // 96 DPI means 96 pixels per 25.4mm. Check the ratio holds within
        // integer-division rounding rather than pinning exact pixel values,
        // so this documents the intended DPI rather than just re-asserting
        // the implementation's arithmetic.
        let size = physical_size_mm(960, 960);
        assert_eq!((size.w, size.h), (254, 254));
    }
}

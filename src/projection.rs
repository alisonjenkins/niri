//! Pure maths for mapping a virtual output's rectangle onto a physical monitor's region.

#![deny(clippy::unwrap_used, clippy::expect_used)]

use std::fmt;

use smithay::utils::{Logical, Point, Rectangle};

/// Which layer of the compositor a projection is rendered into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionKind {
    Overview,
    View,
}

/// A mapping from a rectangle of a virtual output (the source) onto a region
/// of a physical monitor (the viewer).
#[derive(Debug, Clone, PartialEq)]
pub struct Projection {
    pub viewer: String,
    pub source: String,
    pub source_rect: Rectangle<f64, Logical>,
    pub region: Rectangle<f64, Logical>,
    pub kind: ProjectionKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProjectionError {
    EmptySourceRect {
        source_rect: Rectangle<f64, Logical>,
    },
    EmptyRegion {
        region: Rectangle<f64, Logical>,
    },
    AspectRatioMismatch {
        source_aspect: f64,
        region_aspect: f64,
    },
    ViewerIsSource {
        name: String,
    },
}

impl fmt::Display for ProjectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProjectionError::EmptySourceRect { source_rect } => {
                write!(f, "source rect is empty: {source_rect:?}")
            }
            ProjectionError::EmptyRegion { region } => {
                write!(f, "region is empty: {region:?}")
            }
            ProjectionError::AspectRatioMismatch {
                source_aspect,
                region_aspect,
            } => write!(
                f,
                "source aspect ratio {source_aspect} does not match region aspect ratio {region_aspect}"
            ),
            ProjectionError::ViewerIsSource { name } => {
                write!(f, "viewer and source are the same output: {name}")
            }
        }
    }
}

impl std::error::Error for ProjectionError {}

/// Relative tolerance used when comparing the source and region aspect ratios.
const ASPECT_RATIO_TOLERANCE: f64 = 1e-6;

impl Projection {
    pub fn new(
        viewer: String,
        source: String,
        source_rect: Rectangle<f64, Logical>,
        region: Rectangle<f64, Logical>,
        kind: ProjectionKind,
    ) -> Result<Projection, ProjectionError> {
        if source_rect.size.w <= 0. || source_rect.size.h <= 0. {
            return Err(ProjectionError::EmptySourceRect { source_rect });
        }
        if region.size.w <= 0. || region.size.h <= 0. {
            return Err(ProjectionError::EmptyRegion { region });
        }
        if viewer == source {
            return Err(ProjectionError::ViewerIsSource { name: viewer });
        }

        let source_aspect = source_rect.size.w / source_rect.size.h;
        let region_aspect = region.size.w / region.size.h;
        if (source_aspect - region_aspect).abs() > ASPECT_RATIO_TOLERANCE * source_aspect.abs() {
            return Err(ProjectionError::AspectRatioMismatch {
                source_aspect,
                region_aspect,
            });
        }

        Ok(Projection {
            viewer,
            source,
            source_rect,
            region,
            kind,
        })
    }

    /// The scale factor from source-rect coordinates to region coordinates.
    pub fn scale(&self) -> f64 {
        self.region.size.w / self.source_rect.size.w
    }

    /// Maps a point in the viewer's region back to the source rect. Returns
    /// `None` if the point is outside the region.
    pub fn to_source(&self, p: Point<f64, Logical>) -> Option<Point<f64, Logical>> {
        if !self.region.contains(p) {
            return None;
        }
        Some(self.source_rect.loc + (p - self.region.loc).downscale(self.scale()))
    }

    /// Maps a point in the source rect to the viewer's region.
    pub fn to_viewer(&self, p: Point<f64, Logical>) -> Point<f64, Logical> {
        self.region.loc + (p - self.source_rect.loc).upscale(self.scale())
    }
}

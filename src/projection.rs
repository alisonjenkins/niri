//! Pure maths for mapping a virtual output's rectangle onto a physical monitor's region.

#![deny(clippy::unwrap_used, clippy::expect_used)]

use std::fmt;

use smithay::utils::{Logical, Point, Rectangle, Size};

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
        if !is_usable(source_rect) {
            return Err(ProjectionError::EmptySourceRect { source_rect });
        }
        if !is_usable(region) {
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

/// A rectangle with a finite location and a positive, finite size.
fn is_usable(rect: Rectangle<f64, Logical>) -> bool {
    rect.loc.x.is_finite()
        && rect.loc.y.is_finite()
        && rect.size.w.is_finite()
        && rect.size.h.is_finite()
        && rect.size.w > 0.
        && rect.size.h > 0.
}

/// The largest rectangle with `source`'s aspect ratio that fits inside
/// `viewer`, centred within it.
///
/// An empty source gives an empty rectangle at the viewer's centre, which
/// [`Projection::new`] then rejects.
pub fn letterbox(
    source: Size<f64, Logical>,
    viewer: Size<f64, Logical>,
) -> Rectangle<f64, Logical> {
    if !(source.w > 0. && source.h > 0.) {
        return Rectangle::new(Point::from((viewer.w / 2., viewer.h / 2.)), Size::default());
    }
    let scale = (viewer.w / source.w).min(viewer.h / source.h);
    let size = Size::from((source.w * scale, source.h * scale));
    let loc = Point::from(((viewer.w - size.w) / 2., (viewer.h - size.h) / 2.));
    Rectangle::new(loc, size)
}

/// Gap between the viewer strip and the first region, between consecutive
/// regions, and after the last region, as a fraction of the viewer's height.
const OVERVIEW_COLUMN_GAP_FRACTION: f64 = 0.05;
/// Source regions are never shrunk below this fraction of their own size,
/// even if that means overflowing the viewer.
const OVERVIEW_MIN_SOURCE_SCALE: f64 = 0.05;

/// Lays out `sources` as a row of regions to the right of `viewer_strip`,
/// the rectangle where the viewer draws its own overview workspaces.
///
/// The strip itself is never moved or scaled. Regions start one gap right of
/// the strip, one gap apart, centred vertically, at scale 1. If they do not
/// fit before the viewer's right edge (keeping one gap of margin), all of
/// them shrink by the same factor, down to [`OVERVIEW_MIN_SOURCE_SCALE`].
/// When the strip already fills the viewer (overview closed, zoom 1) the
/// regions lie fully off-screen to the right at scale 1.
pub fn overview_columns(
    viewer_strip: Rectangle<f64, Logical>,
    viewer_size: Size<f64, Logical>,
    sources: &[Size<f64, Logical>],
) -> Vec<Rectangle<f64, Logical>> {
    let gap = OVERVIEW_COLUMN_GAP_FRACTION * viewer_size.h;

    let (start_x, source_scale) = if viewer_strip.size.w >= viewer_size.w {
        (viewer_size.w, 1.)
    } else {
        let strip_right = viewer_strip.loc.x + viewer_strip.size.w;
        let sources_width: f64 = sources.iter().map(|s| s.w).sum();
        let gaps_and_margin = gap * (sources.len() as f64 + 1.);
        let room = viewer_size.w - strip_right - gaps_and_margin;
        let scale = if sources_width <= room || sources_width <= 0. {
            1.
        } else {
            (room / sources_width).max(OVERVIEW_MIN_SOURCE_SCALE)
        };
        (strip_right, scale)
    };

    let mut cursor = start_x;
    sources
        .iter()
        .map(|source| {
            let size = Size::from((source.w * source_scale, source.h * source_scale));
            cursor += gap;
            let loc = Point::from((cursor, (viewer_size.h - size.h) / 2.));
            cursor += size.w;
            Rectangle::new(loc, size)
        })
        .collect()
}

/// Which projections exist and which one, if any, is currently being viewed.
#[derive(Debug, Default)]
pub struct ProjectionState {
    pub projections: Vec<Projection>,
    pub viewing: Option<Viewing>,
}

/// A viewer currently viewing a source's projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Viewing {
    pub viewer: String,
    pub source: String,
}

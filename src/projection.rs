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

/// The largest rectangle with `source`'s aspect ratio that fits inside
/// `viewer`, centred within it.
pub fn letterbox(
    source: Size<f64, Logical>,
    viewer: Size<f64, Logical>,
) -> Rectangle<f64, Logical> {
    let scale = (viewer.w / source.w).min(viewer.h / source.h);
    let size = Size::from((source.w * scale, source.h * scale));
    let loc = Point::from(((viewer.w - size.w) / 2., (viewer.h - size.h) / 2.));
    Rectangle::new(loc, size)
}

/// Gap between the viewer strip and the first region, and between
/// consecutive regions, as a fraction of the viewer's height.
const OVERVIEW_COLUMN_GAP_FRACTION: f64 = 0.05;
/// The row (strip + gaps + regions) is shrunk to fit within this fraction of
/// the viewer's width when it would otherwise overflow.
const OVERVIEW_ROW_FIT_FRACTION: f64 = 0.95;
/// The row of source regions is never shrunk below this fraction of their
/// own size, even if that means overflowing the target row width.
const OVERVIEW_MIN_SOURCE_SCALE: f64 = 0.05;

/// The result of laying out other viewers' source regions next to a
/// viewer's own overview strip.
#[derive(Debug, Clone, PartialEq)]
pub struct OverviewColumns {
    /// How far the caller must shift `viewer_strip` horizontally so the
    /// whole row (strip + gaps + regions) is centred on the viewer. The
    /// regions below are already positioned as if that shift had been
    /// applied.
    pub strip_offset_x: f64,
    pub regions: Vec<Rectangle<f64, Logical>>,
}

/// Lays out `sources` as a row of regions to the right of `viewer_strip`,
/// for a viewer whose own overview workspaces are drawn at `viewer_strip`.
///
/// Each region starts at scale 1 (its own size) and regions are separated by
/// a gap proportional to `viewer_size`'s height; the whole row is centred
/// horizontally on the viewer, shrinking only the regions (never the strip)
/// if the unshrunk row would not fit. When the strip already fills the
/// viewer (overview closed, zoom 1) the regions are instead placed fully
/// off-screen to the right, with no centring or shrinking.
pub fn overview_columns(
    viewer_strip: Rectangle<f64, Logical>,
    viewer_size: Size<f64, Logical>,
    sources: &[Size<f64, Logical>],
) -> OverviewColumns {
    let gap = OVERVIEW_COLUMN_GAP_FRACTION * viewer_size.h;

    if viewer_strip.size.w >= viewer_size.w {
        let mut cursor = viewer_size.w;
        let regions = sources
            .iter()
            .map(|source| {
                cursor += gap;
                let loc = Point::from((cursor, (viewer_size.h - source.h) / 2.));
                cursor += source.w;
                Rectangle::new(loc, *source)
            })
            .collect();

        return OverviewColumns {
            strip_offset_x: 0.,
            regions,
        };
    }

    let unshrunk_sources_width: f64 = sources.iter().map(|s| s.w).sum();
    let gap_total = gap * sources.len() as f64;
    let unshrunk_row_width = viewer_strip.size.w + gap_total + unshrunk_sources_width;

    let target_row_width = OVERVIEW_ROW_FIT_FRACTION * viewer_size.w;
    let source_scale = if unshrunk_row_width <= target_row_width || unshrunk_sources_width <= 0. {
        1.
    } else {
        let available_for_sources = target_row_width - viewer_strip.size.w - gap_total;
        (available_for_sources / unshrunk_sources_width).max(OVERVIEW_MIN_SOURCE_SCALE)
    };

    let sources_width: f64 = sources.iter().map(|s| s.w * source_scale).sum();
    let row_width = viewer_strip.size.w + gap_total + sources_width;
    let strip_offset_x = (viewer_size.w - row_width) / 2. - viewer_strip.loc.x;

    let mut cursor = viewer_strip.loc.x + strip_offset_x + viewer_strip.size.w;
    let regions = sources
        .iter()
        .map(|source| {
            let size = Size::from((source.w * source_scale, source.h * source_scale));
            cursor += gap;
            let loc = Point::from((cursor, (viewer_size.h - size.h) / 2.));
            cursor += size.w;
            Rectangle::new(loc, size)
        })
        .collect();

    OverviewColumns {
        strip_offset_x,
        regions,
    }
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

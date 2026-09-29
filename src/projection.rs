//! Pure maths for mapping a virtual output's rectangle onto a physical monitor's region.

#![deny(clippy::unwrap_used, clippy::expect_used)]

use std::fmt;
use std::time::Duration;

use smithay::utils::{Logical, Point, Rectangle, Size};

use crate::layout::workspace::WorkspaceId;

/// Which layer of the compositor a projection is rendered into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionKind {
    View,
    /// One workspace of a virtual output, shown as a tile in the overview band's column.
    Tile {
        workspace: WorkspaceId,
    },
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

/// Width of the overview band, as a fraction of the physical monitor's width, at full overview
/// progress.
pub const BAND_FRACTION: f64 = 0.15;

/// The band a physical monitor reserves at its right edge for virtual outputs' workspaces,
/// while the overview is open.
///
/// Full height, right-aligned; width grows from 0 at `progress` 0 to `BAND_FRACTION` of
/// `viewer_size.w` at `progress` 1. `progress` is clamped to `[0, 1]`; non-finite is treated
/// as 0.
pub fn band_rect(viewer_size: Size<f64, Logical>, progress: f64) -> Rectangle<f64, Logical> {
    let progress = if progress.is_finite() {
        progress.clamp(0., 1.)
    } else {
        0.
    };
    let width = BAND_FRACTION * viewer_size.w * progress;
    let loc = Point::from((viewer_size.w - width, 0.));
    Rectangle::new(loc, Size::from((width, viewer_size.h)))
}

/// A virtual output whose workspaces the column lists.
#[derive(Debug, Clone)]
pub struct ColumnSource {
    pub name: String,
    pub output_size: Size<f64, Logical>,
    /// Number of workspaces, including the empty last one.
    pub workspace_count: usize,
    pub active_workspace: usize,
}

/// A source's name label plus its tiles, laid out in the band's column.
#[derive(Debug, Clone)]
pub struct GroupLayout {
    /// Index of the source in the slice passed to [`column_layout`].
    pub source: usize,
    pub name: String,
    pub label_rect: Rectangle<f64, Logical>,
    pub tiles: Vec<TileLayout>,
}

/// One workspace's tile in the band's column.
#[derive(Debug, Clone)]
pub struct TileLayout {
    /// Index of the owning group, same as [`GroupLayout::source`].
    pub group: usize,
    pub workspace: usize,
    pub region: Rectangle<f64, Logical>,
    pub active: bool,
}

/// Result of laying out a band's column.
#[derive(Debug, Clone, Default)]
pub struct ColumnLayout {
    pub groups: Vec<GroupLayout>,
    /// Total height of labels, tiles and gaps, before clamping or centring.
    pub content_h: f64,
    /// The scroll offset actually used, after clamping.
    pub scroll: f64,
    /// Indices into the `sources` slice that were skipped (bad size or no workspaces).
    pub skipped: Vec<usize>,
}

/// How far one mouse wheel detent scrolls the band's column, in logical pixels.
pub const COLUMN_WHEEL_STEP: f64 = 120.;

/// Height of a group's name-label row, in logical pixels.
const COLUMN_LABEL_HEIGHT: f64 = 24.;
/// Horizontal margin on each side of a tile within the band, in logical pixels.
const COLUMN_TILE_MARGIN: f64 = 8.;
/// Extra vertical gap after a group's last tile, on top of the ordinary tile gap, so groups
/// read as visually distinct from the workspaces within them.
const COLUMN_GROUP_GAP: f64 = 2. * COLUMN_LABEL_HEIGHT;

/// Lays out `sources` as a single scrollable column inside `band`: for each source, a label
/// row followed by one tile per workspace, all at the same width (`band.size.w` minus
/// `2 * COLUMN_TILE_MARGIN`), height following the source's aspect ratio. The gap between
/// tiles within a group is niri's overview workspace-gap ratio, `0.1` of the tile height;
/// groups are separated by that plus [`COLUMN_GROUP_GAP`].
///
/// Positions are in the viewer's logical coordinates inside `band`, offset by `-scroll` (and,
/// when the content is shorter than the band, centred instead). `scroll` is clamped to
/// `[0, max(0, content_h - band.size.h)]`; non-finite is treated as 0.
///
/// Sources with a zero, negative or non-finite size, or zero workspaces, are skipped; their
/// indices are returned in `skipped` for the caller to log. A zero-width band gives an empty
/// layout.
pub fn column_layout(
    band: Rectangle<f64, Logical>,
    sources: &[ColumnSource],
    scroll: f64,
) -> ColumnLayout {
    let tile_w = band.size.w - 2. * COLUMN_TILE_MARGIN;
    if !(band.size.w > 0. && tile_w > 0.) {
        return ColumnLayout {
            skipped: (0..sources.len()).collect(),
            ..Default::default()
        };
    }

    let mut groups = Vec::new();
    let mut skipped = Vec::new();
    let mut y = 0.;

    for (i, src) in sources.iter().enumerate() {
        if !(src.output_size.w.is_finite()
            && src.output_size.h.is_finite()
            && src.output_size.w > 0.
            && src.output_size.h > 0.)
            || src.workspace_count == 0
        {
            skipped.push(i);
            continue;
        }

        let tile_h = tile_w * src.output_size.h / src.output_size.w;
        let tile_gap = 0.1 * tile_h;

        let label_rect = Rectangle::new(
            Point::from((band.loc.x, y)),
            Size::from((band.size.w, COLUMN_LABEL_HEIGHT)),
        );
        y += COLUMN_LABEL_HEIGHT;

        let mut tiles = Vec::with_capacity(src.workspace_count);
        for ws in 0..src.workspace_count {
            let region = Rectangle::new(
                Point::from((band.loc.x + COLUMN_TILE_MARGIN, y)),
                Size::from((tile_w, tile_h)),
            );
            tiles.push(TileLayout {
                group: i,
                workspace: ws,
                region,
                active: ws == src.active_workspace,
            });
            y += tile_h;
            if ws + 1 < src.workspace_count {
                y += tile_gap;
            }
        }

        groups.push(GroupLayout {
            source: i,
            name: src.name.clone(),
            label_rect,
            tiles,
        });

        y += COLUMN_GROUP_GAP;
    }
    if !groups.is_empty() {
        y -= COLUMN_GROUP_GAP;
    }
    let content_h = y.max(0.);

    let offset = if content_h < band.size.h {
        (band.size.h - content_h) / 2.
    } else {
        0.
    };
    let scroll = if content_h < band.size.h {
        0.
    } else if scroll.is_finite() {
        scroll.clamp(0., (content_h - band.size.h).max(0.))
    } else {
        0.
    };

    let shift = offset - scroll;
    for group in &mut groups {
        group.label_rect.loc.y += shift;
        for tile in &mut group.tiles {
            tile.region.loc.y += shift;
        }
    }

    ColumnLayout {
        groups,
        content_h,
        scroll,
        skipped,
    }
}

/// The scroll offset, starting from `current_scroll`, that brings `target` (a `(source index,
/// workspace index)` pair, matching [`GroupLayout::source`] / [`TileLayout::workspace`]) fully
/// into `band`'s view, moving as little as possible. Returns `current_scroll` unchanged (after
/// clamping) if `target` is already fully visible, or if it does not exist (an unknown or
/// skipped source).
pub fn scroll_to_show(
    band: Rectangle<f64, Logical>,
    sources: &[ColumnSource],
    current_scroll: f64,
    target: (usize, usize),
) -> f64 {
    let unscrolled = column_layout(band, sources, 0.);
    let max_scroll = (unscrolled.content_h - band.size.h).max(0.);
    let current = if current_scroll.is_finite() {
        current_scroll.clamp(0., max_scroll)
    } else {
        0.
    };

    let Some(tile) = unscrolled
        .groups
        .iter()
        .find(|g| g.source == target.0)
        .and_then(|g| g.tiles.iter().find(|t| t.workspace == target.1))
    else {
        return current;
    };

    let top = tile.region.loc.y;
    let bottom = top + tile.region.size.h;

    let new_scroll = if top - current < 0. {
        top
    } else if bottom - current > band.size.h {
        bottom - band.size.h
    } else {
        current
    };

    new_scroll.clamp(0., max_scroll)
}

/// How fast and which way a drag held at height `y` in a band of `height` scrolls its column,
/// from -1 (up, at the top edge) to 1 (down, at the bottom edge), and 0 outside the
/// `trigger_height` zones at either edge. The same zones as the physical monitor's DnD edge
/// workspace switch.
pub fn edge_scroll_factor(y: f64, height: f64, trigger_height: f64) -> f64 {
    if !(y.is_finite() && height.is_finite() && trigger_height.is_finite()) || height <= 0. {
        return 0.;
    }
    let y = y.clamp(0., height);
    let trigger_height = trigger_height.clamp(0., height / 2.);
    // Sanity check for trigger-height 0 or small bands.
    if trigger_height < 0.01 {
        return 0.;
    }
    let delta = if y < trigger_height {
        -(trigger_height - y)
    } else if height - y < trigger_height {
        trigger_height - (height - y)
    } else {
        0.
    };
    delta / trigger_height
}

/// A window dragged over a band, which scrolls the column while held near its top or bottom.
#[derive(Debug, Clone, PartialEq)]
pub struct BandEdgeScroll {
    pub viewer: String,
    /// The pointer's height within the band.
    pub y: f64,
    /// When the column last advanced.
    pub last_time: Option<Duration>,
    /// When the pointer entered an edge zone, for the start delay.
    pub nonzero_start: Option<Duration>,
    /// Whether the column is moving or about to, so frames must keep coming.
    pub active: bool,
}

/// The band a physical monitor shows virtual outputs' workspaces in while the overview is open.
#[derive(Debug, Clone)]
pub struct Band {
    pub viewer: String,
    /// The part of the band on screen, local to the viewer; its width follows the overview
    /// progress.
    pub rect: Rectangle<f64, Logical>,
    /// Laid out at the fully open band's width from `rect`'s left edge, so the column slides in
    /// with the band instead of resizing on every frame of the animation.
    pub column: ColumnLayout,
    /// Sources left out of the column for an unusable size or no workspaces.
    pub skipped: Vec<String>,
}

/// Which projections exist and which one, if any, is currently being viewed.
#[derive(Debug, Default)]
pub struct ProjectionState {
    pub projections: Vec<Projection>,
    pub viewing: Option<Viewing>,
    /// At most one per physical monitor; none while locked or with no virtual output on.
    pub bands: Vec<Band>,
}

/// A viewer currently viewing a source's projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Viewing {
    pub viewer: String,
    pub source: String,
    pub origin: ViewOrigin,
}

/// How view mode was entered, which decides whether Escape leaves it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewOrigin {
    /// `niri msg view-output` or the `view-output` bind. Escape belongs to the viewed app.
    Command,
    /// Clicking or tapping a virtual output's workspace in the overview. Escape returns to
    /// the viewer, since the user only passed through the virtual output from there.
    Overview,
}

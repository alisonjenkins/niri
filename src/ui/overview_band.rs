//! What the overview band draws around its tiles: the opaque fill, each virtual output's name
//! and the highlight on its active workspace.

use std::cell::RefCell;
use std::collections::HashMap;

use niri_config::Color;
use ordered_float::NotNan;
use smithay::backend::renderer::element::Kind;
use smithay::backend::renderer::gles::GlesTexture;
use smithay::utils::{Logical, Point, Size};
use tracing::warn;

use crate::projection::Band;
use crate::render_helpers::primary_gpu_texture::PrimaryGpuTextureRenderElement;
use crate::render_helpers::renderer::NiriRenderer;
use crate::render_helpers::solid_color::{SolidColorBuffer, SolidColorRenderElement};
use crate::render_helpers::texture::{TextureBuffer, TextureRenderElement};
use crate::ui::text_texture::render_text_texture;

/// Keeps the label within the column's label row.
const PADDING: i32 = 2;
const FONT: &str = "sans 14px";
/// Space between a label and the band's left edge, in logical pixels.
const LABEL_INDENT: f64 = 8.;
/// Opaque, so the viewer's own windows scrolled under the band never show through it.
const FILL_COLOR: [f32; 4] = [0., 0., 0., 1.];
/// Thinnest highlight drawn, so a zero-width focus ring still shows which workspace the virtual
/// output shows.
const MIN_HIGHLIGHT_WIDTH: f64 = 1.;

/// Cached label textures per (source name, output scale). A failed render
/// is cached as `None` so it is reported once, not every frame.
type LabelCache = HashMap<(String, NotNan<f64>), Option<TextureBuffer<GlesTexture>>>;

/// The highlight's four edges: top, bottom, left, right.
type HighlightEdges = [SolidColorBuffer; 4];

#[derive(Default)]
pub struct OverviewBand {
    labels: RefCell<LabelCache>,
    /// Keyed by viewer.
    fills: HashMap<String, SolidColorBuffer>,
    /// Keyed by (viewer, source).
    highlights: HashMap<(String, String), HighlightEdges>,
}

impl OverviewBand {
    /// Sizes the fills and highlights for `bands` and drops what no band uses any more.
    pub fn update(&mut self, bands: &[Band], color: Color, width: f64) {
        self.fills
            .retain(|viewer, _| bands.iter().any(|band| &band.viewer == viewer));
        for band in bands {
            self.fills
                .entry(band.viewer.clone())
                .or_insert_with(|| SolidColorBuffer::new(band.rect.size, FILL_COLOR))
                .resize(band.rect.size);
        }

        let width = if width.is_finite() {
            width.max(MIN_HIGHLIGHT_WIDTH)
        } else {
            MIN_HIGHLIGHT_WIDTH
        };
        let mut used = Vec::new();
        for band in bands {
            for group in &band.column.groups {
                let Some(tile) = group.tiles.iter().find(|tile| tile.active) else {
                    continue;
                };
                let key = (band.viewer.clone(), group.name.clone());
                let [top, bottom, left, right] = edge_sizes(tile.region.size, width);
                let edges = self.highlights.entry(key.clone()).or_insert_with(|| {
                    [top, bottom, left, right].map(|size| SolidColorBuffer::new(size, color))
                });
                for (edge, size) in edges.iter_mut().zip([top, bottom, left, right]) {
                    edge.update(size, color);
                }
                used.push(key);
            }
        }
        self.highlights.retain(|key, _| used.contains(key));

        self.labels.get_mut().retain(|(name, _), _| {
            bands
                .iter()
                .any(|band| band.column.groups.iter().any(|group| &group.name == name))
        });
    }

    /// The opaque fill behind `band`'s column.
    pub fn render_fill(&self, band: &Band) -> Option<SolidColorRenderElement> {
        let buffer = self.fills.get(&band.viewer)?;
        Some(SolidColorRenderElement::from_buffer(
            buffer,
            band.rect.loc,
            1.,
            Kind::Unspecified,
        ))
    }

    /// A border inside each source's active tile, where that tile is on screen.
    pub fn render_highlights(&self, band: &Band, push: &mut dyn FnMut(SolidColorRenderElement)) {
        for group in &band.column.groups {
            let Some(tile) = group.tiles.iter().find(|tile| tile.active) else {
                continue;
            };
            if tile.region.intersection(band.rect).is_none() {
                continue;
            }
            let Some(edges) = self
                .highlights
                .get(&(band.viewer.clone(), group.name.clone()))
            else {
                continue;
            };

            let region = tile.region;
            let [top, bottom, left, right] = edges;
            let right_x = region.loc.x + region.size.w - right.size().w;
            let bottom_y = region.loc.y + region.size.h - bottom.size().h;
            let below_top = region.loc.y + top.size().h;
            for (buffer, loc) in [
                (top, region.loc),
                (bottom, Point::from((region.loc.x, bottom_y))),
                (left, Point::from((region.loc.x, below_top))),
                (right, Point::from((right_x, below_top))),
            ] {
                push(SolidColorRenderElement::from_buffer(
                    buffer,
                    loc,
                    1.,
                    Kind::Unspecified,
                ));
            }
        }
    }

    /// Each group's name in its label row, where the row is on screen and wide enough.
    pub fn render_labels<R: NiriRenderer>(
        &self,
        renderer: &mut R,
        band: &Band,
        scale: f64,
        push: &mut dyn FnMut(PrimaryGpuTextureRenderElement),
    ) {
        let Ok(scale_key) = NotNan::new(scale) else {
            return;
        };

        for group in &band.column.groups {
            let row = group.label_rect;
            if row.intersection(band.rect).is_none() {
                continue;
            }

            let buffer = self
                .labels
                .borrow_mut()
                .entry((group.name.clone(), scale_key))
                .or_insert_with(|| {
                    render_text_texture(
                        renderer.as_gles_renderer(),
                        scale,
                        &group.name,
                        FONT,
                        PADDING,
                    )
                    .inspect_err(|error| {
                        warn!(
                            source = %group.name,
                            scale,
                            %error,
                            "failed to render overview band label"
                        );
                    })
                    .ok()
                })
                .clone();
            let Some(buffer) = buffer else {
                continue;
            };

            let size = buffer.logical_size();
            if size.w + LABEL_INDENT > row.size.w {
                continue;
            }
            let location = Point::from((
                row.loc.x + LABEL_INDENT,
                row.loc.y + (row.size.h - size.h) / 2.,
            ))
            .to_physical_precise_round(scale)
            .to_logical(scale);

            let elem = TextureRenderElement::from_texture_buffer(
                buffer,
                location,
                1.,
                None,
                None,
                Kind::Unspecified,
            );
            push(PrimaryGpuTextureRenderElement(elem));
        }
    }
}

/// Sizes of a highlight's top, bottom, left and right edges inside a tile of `size`.
fn edge_sizes(size: Size<f64, Logical>, width: f64) -> [Size<f64, Logical>; 4] {
    let across = width.min(size.w / 2.);
    let down = width.min(size.h / 2.);
    let side_h = (size.h - 2. * down).max(0.);
    [
        Size::from((size.w, down)),
        Size::from((size.w, down)),
        Size::from((across, side_h)),
        Size::from((across, side_h)),
    ]
}

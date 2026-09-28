//! The source name drawn above each overview projection column.

use std::cell::RefCell;
use std::collections::HashMap;

use ordered_float::NotNan;
use smithay::backend::renderer::element::Kind;
use smithay::backend::renderer::gles::GlesTexture;
use smithay::utils::{Logical, Point, Rectangle};
use tracing::warn;

use crate::render_helpers::primary_gpu_texture::PrimaryGpuTextureRenderElement;
use crate::render_helpers::renderer::NiriRenderer;
use crate::render_helpers::texture::{TextureBuffer, TextureRenderElement};
use crate::ui::text_texture::render_text_texture;

const PADDING: i32 = 4;
const FONT: &str = "sans 14px";
/// Space between the label and the top of its column, in logical pixels.
const GAP: f64 = 8.;

/// Cached label textures per (source name, output scale). A failed render
/// is cached as `None` so it is reported once, not every frame.
type LabelCache = HashMap<(String, NotNan<f64>), Option<TextureBuffer<GlesTexture>>>;

#[derive(Default)]
pub struct OverviewColumnLabels {
    buffers: RefCell<LabelCache>,
}

impl OverviewColumnLabels {
    /// Drops cached labels for sources `keep` rejects.
    pub fn retain_sources(&mut self, keep: impl Fn(&str) -> bool) {
        self.buffers.get_mut().retain(|(name, _), _| keep(name));
    }

    /// The label for `source`, centred above `region` (viewer logical
    /// coordinates) and kept on screen. `None` if the label could not be
    /// rendered.
    pub fn render<R: NiriRenderer>(
        &self,
        renderer: &mut R,
        source: &str,
        scale: f64,
        region: Rectangle<f64, Logical>,
    ) -> Option<PrimaryGpuTextureRenderElement> {
        let scale_key = NotNan::new(scale).ok()?;

        let mut buffers = self.buffers.borrow_mut();
        let buffer = buffers
            .entry((source.to_owned(), scale_key))
            .or_insert_with(|| {
                match render_text_texture(renderer.as_gles_renderer(), scale, source, FONT, PADDING)
                {
                    Ok(buffer) => Some(buffer),
                    Err(error) => {
                        warn!(source, scale, %error, "failed to render overview column label");
                        None
                    }
                }
            });
        let buffer = buffer.clone()?;

        let size = buffer.logical_size();
        let x = region.loc.x + (region.size.w - size.w).max(0.) / 2.;
        let y = (region.loc.y - GAP - size.h).max(0.);
        let location = Point::from((x, y))
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
        Some(PrimaryGpuTextureRenderElement(elem))
    }
}

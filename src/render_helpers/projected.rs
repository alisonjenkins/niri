//! An element rendered for one output (the source) and shown on another (the viewer).

use smithay::backend::renderer::element::{Element, Id, Kind, RenderElement, UnderlyingStorage};
use smithay::backend::renderer::utils::{CommitCounter, DamageSet, OpaqueRegions};
use smithay::backend::renderer::Renderer;
use smithay::utils::user_data::UserDataMap;
use smithay::utils::{Buffer, Logical, Physical, Point, Rectangle, Scale, Transform};

/// A source output's element, moved and scaled into a region of a viewer
/// output.
///
/// The inner element is always asked for its geometry at the source's scale,
/// whatever scale the viewer renders at, so the source's own physical
/// layout is kept and then mapped as a whole. Stacking smithay's Rescale and
/// Relocate wrappers cannot do that: they pass the viewer's scale through to
/// the inner element, which sizes itself at that scale while its location
/// was computed at the source's, so the two drift apart whenever the scales
/// differ.
///
/// The inner element is boxed because the source's elements are the same
/// enum this wraps into, which would otherwise be infinitely sized.
#[derive(Debug)]
pub struct ProjectedElement<E> {
    inner: Box<E>,
    source_scale: Scale<f64>,
    /// Top-left of the shown source area, in source physical pixels.
    source_origin: Point<i32, Physical>,
    /// Top-left of the region on the viewer, in viewer physical pixels.
    target_origin: Point<i32, Physical>,
    /// Source physical pixels to viewer physical pixels.
    factor: f64,
}

impl<E: Element> ProjectedElement<E> {
    /// Wraps `inner`, an element of the source, to show the source's
    /// `source_rect` at `region` on the viewer. `region` is expected to have
    /// the same aspect ratio as `source_rect`.
    pub fn new(
        inner: E,
        source_scale: f64,
        source_rect: Rectangle<f64, Logical>,
        viewer_scale: f64,
        region: Rectangle<f64, Logical>,
    ) -> Self {
        let projection_scale = region.size.w / source_rect.size.w;
        Self {
            inner: Box::new(inner),
            source_scale: Scale::from(source_scale),
            source_origin: source_rect.loc.to_physical_precise_round(source_scale),
            target_origin: region.loc.to_physical_precise_round(viewer_scale),
            factor: projection_scale * viewer_scale / source_scale,
        }
    }

    fn scale_rect(&self, rect: Rectangle<i32, Physical>) -> Rectangle<i32, Physical> {
        rect.to_f64().upscale(self.factor).to_i32_round()
    }
}

impl<E: Element> Element for ProjectedElement<E> {
    fn id(&self) -> &Id {
        self.inner.id()
    }

    fn current_commit(&self) -> CommitCounter {
        self.inner.current_commit()
    }

    fn src(&self) -> Rectangle<f64, Buffer> {
        self.inner.src()
    }

    fn geometry(&self, _scale: Scale<f64>) -> Rectangle<i32, Physical> {
        let mut geo = self.inner.geometry(self.source_scale);
        geo.loc -= self.source_origin;
        let mut geo = self.scale_rect(geo);
        geo.loc += self.target_origin;
        geo
    }

    fn transform(&self) -> Transform {
        self.inner.transform()
    }

    fn damage_since(
        &self,
        _scale: Scale<f64>,
        commit: Option<CommitCounter>,
    ) -> DamageSet<i32, Physical> {
        self.inner
            .damage_since(self.source_scale, commit)
            .into_iter()
            .map(|rect| rect.to_f64().upscale(self.factor).to_i32_up())
            .collect()
    }

    fn opaque_regions(&self, _scale: Scale<f64>) -> OpaqueRegions<i32, Physical> {
        self.inner
            .opaque_regions(self.source_scale)
            .into_iter()
            .map(|rect| self.scale_rect(rect))
            .collect()
    }

    fn alpha(&self) -> f32 {
        self.inner.alpha()
    }

    fn kind(&self) -> Kind {
        self.inner.kind()
    }

    fn is_framebuffer_effect(&self) -> bool {
        self.inner.is_framebuffer_effect()
    }
}

impl<R: Renderer, E: RenderElement<R>> RenderElement<R> for ProjectedElement<E> {
    fn draw(
        &self,
        frame: &mut R::Frame<'_, '_>,
        src: Rectangle<f64, Buffer>,
        dst: Rectangle<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
        opaque_regions: &[Rectangle<i32, Physical>],
        cache: Option<&UserDataMap>,
    ) -> Result<(), R::Error> {
        self.inner
            .draw(frame, src, dst, damage, opaque_regions, cache)
    }

    fn underlying_storage(&self, renderer: &mut R) -> Option<UnderlyingStorage<'_>> {
        self.inner.underlying_storage(renderer)
    }

    fn capture_framebuffer(
        &self,
        frame: &mut R::Frame<'_, '_>,
        src: Rectangle<f64, Buffer>,
        dst: Rectangle<i32, Physical>,
        cache: &UserDataMap,
    ) -> Result<(), R::Error> {
        self.inner.capture_framebuffer(frame, src, dst, cache)
    }
}

#[cfg(test)]
mod tests {
    use smithay::utils::Size;

    use super::*;
    use crate::render_helpers::solid_color::{SolidColorBuffer, SolidColorRenderElement};

    fn element(x: f64, y: f64, w: f64, h: f64) -> SolidColorRenderElement {
        let buffer = SolidColorBuffer::new((w, h), [1., 1., 1., 1.]);
        SolidColorRenderElement::from_buffer(&buffer, (x, y), 1., Kind::Unspecified)
    }

    fn rect(x: f64, y: f64, w: f64, h: f64) -> Rectangle<f64, Logical> {
        Rectangle::new(Point::from((x, y)), Size::from((w, h)))
    }

    #[test]
    fn maps_source_geometry_into_the_region() {
        // Source area (100, 0) 400x300 shown at half size at (1000, 50).
        let projected = ProjectedElement::new(
            element(200., 100., 40., 20.),
            1.,
            rect(100., 0., 400., 300.),
            1.,
            rect(1000., 50., 200., 150.),
        );

        assert_eq!(
            projected.geometry(Scale::from(1.)),
            Rectangle::new(Point::from((1050, 100)), Size::from((20, 10)))
        );
    }

    #[test]
    fn ignores_the_viewer_scale_passed_at_render_time() {
        // Source at scale 1, viewer at scale 2, projection scale 1: the
        // element keeps its logical size and so doubles in physical pixels.
        let projected = ProjectedElement::new(
            element(10., 20., 40., 20.),
            1.,
            rect(0., 0., 400., 300.),
            2.,
            rect(100., 50., 400., 300.),
        );

        assert_eq!(
            projected.geometry(Scale::from(2.)),
            Rectangle::new(Point::from((220, 140)), Size::from((80, 40)))
        );
    }
}

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use niri_config::Config;
use ordered_float::NotNan;
use pangocairo::cairo::{self, ImageSurface};
use pangocairo::pango::FontDescription;
use smithay::backend::renderer::element::Kind;
use smithay::backend::renderer::gles::{GlesRenderer, GlesTexture};
use smithay::output::Output;
use smithay::reexports::gbm::Format as Fourcc;
use smithay::utils::{Point, Transform};
use tracing::{debug, warn};

use crate::animation::{Animation, Clock};
use crate::render_helpers::primary_gpu_texture::PrimaryGpuTextureRenderElement;
use crate::render_helpers::renderer::NiriRenderer;
use crate::render_helpers::texture::{TextureBuffer, TextureRenderElement};
use crate::utils::{output_size, to_physical_precise_round};

const PADDING: i32 = 8;
const FONT: &str = "sans 14px";

/// How long the label stays fully shown before it starts fading out.
///
/// Matches the attention window niri gives other transient notifications
/// (see `config_error_notification`'s `Shown` state).
const VIEW_OUTPUT_LABEL_DURATION: Duration = Duration::from_secs(2);

pub struct ViewOutputLabel {
    state: State,
    text: String,
    buffers: RefCell<HashMap<NotNan<f64>, Option<TextureBuffer<GlesTexture>>>>,

    clock: Clock,
    config: Rc<RefCell<Config>>,
}

enum State {
    Hidden,
    Showing(Animation),
    Shown(Duration),
    Hiding(Animation),
}

impl ViewOutputLabel {
    pub fn new(clock: Clock, config: Rc<RefCell<Config>>) -> Self {
        Self {
            state: State::Hidden,
            text: String::new(),
            buffers: RefCell::new(HashMap::new()),
            clock,
            config,
        }
    }

    fn state_label(&self) -> &'static str {
        match self.state {
            State::Hidden => "hidden",
            State::Showing(_) => "showing",
            State::Shown(_) => "shown",
            State::Hiding(_) => "hiding",
        }
    }

    fn animation(&self, from: f64, to: f64) -> Animation {
        let c = self.config.borrow();
        Animation::new(
            self.clock.clone(),
            from,
            to,
            0.,
            c.animations.config_notification_open_close.0,
        )
    }

    pub fn show(&mut self, text: String) {
        debug!(
            text,
            previous_state = self.state_label(),
            "showing view-output label"
        );

        if self.text != text {
            self.text = text;
            self.buffers.borrow_mut().clear();
        }

        self.state = State::Showing(self.animation(0., 1.));
    }

    pub fn hide(&mut self) {
        if matches!(self.state, State::Hidden) {
            return;
        }

        debug!(
            previous_state = self.state_label(),
            "hiding view-output label"
        );

        self.state = State::Hiding(self.animation(1., 0.));
    }

    pub fn advance_animations(&mut self) {
        match &mut self.state {
            State::Hidden => (),
            State::Showing(anim) => {
                if anim.is_done() {
                    self.state =
                        State::Shown(self.clock.now_unadjusted() + VIEW_OUTPUT_LABEL_DURATION);
                }
            }
            State::Shown(deadline) => {
                if self.clock.now_unadjusted() >= *deadline {
                    self.hide();
                }
            }
            State::Hiding(anim) => {
                if anim.is_clamped_done() {
                    self.state = State::Hidden;
                }
            }
        }
    }

    pub fn are_animations_ongoing(&self) -> bool {
        !matches!(self.state, State::Hidden)
    }

    pub fn render<R: NiriRenderer>(
        &self,
        renderer: &mut R,
        output: &Output,
    ) -> Option<PrimaryGpuTextureRenderElement> {
        if matches!(self.state, State::Hidden) {
            return None;
        }

        let scale = output.current_scale().fractional_scale();
        let scale_key = NotNan::new(scale).ok()?;
        let output_size = output_size(output);
        let text = self.text.clone();
        let text_len = text.len();

        let mut buffers = self.buffers.borrow_mut();
        let buffer = buffers.entry(scale_key).or_insert_with(move || {
            match render(renderer.as_gles_renderer(), scale, &text) {
                Ok(buffer) => Some(buffer),
                Err(error) => {
                    warn!(text_len, scale, %error, "failed to render view-output label");
                    None
                }
            }
        });
        let buffer = buffer.clone()?;

        let size = buffer.logical_size();
        let y_range = size.h + f64::from(PADDING) * 2.;

        let x = (output_size.w - size.w).max(0.) / 2.;
        let y = match &self.state {
            State::Hidden => return None,
            State::Showing(anim) | State::Hiding(anim) => -size.h + anim.value() * y_range,
            State::Shown(_) => f64::from(PADDING) * 2.,
        };

        let location = Point::from((x, y));
        let location = location.to_physical_precise_round(scale).to_logical(scale);

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

fn render(
    renderer: &mut GlesRenderer,
    scale: f64,
    text: &str,
) -> anyhow::Result<TextureBuffer<GlesTexture>> {
    let _span = tracy_client::span!("view_output_label::render");

    let padding: i32 = to_physical_precise_round(scale, PADDING);

    let mut font = FontDescription::from_string(FONT);
    font.set_absolute_size(to_physical_precise_round(scale, font.size()));

    let surface = ImageSurface::create(cairo::Format::ARgb32, 0, 0)?;
    let cr = cairo::Context::new(&surface)?;
    let layout = pangocairo::functions::create_layout(&cr);
    layout.context().set_round_glyph_positions(false);
    layout.set_font_description(Some(&font));
    layout.set_text(text);

    let (mut width, mut height) = layout.pixel_size();
    width += padding * 2;
    height += padding * 2;

    let surface = ImageSurface::create(cairo::Format::ARgb32, width, height)?;
    let cr = cairo::Context::new(&surface)?;
    cr.set_source_rgb(0.1, 0.1, 0.1);
    cr.paint()?;

    cr.move_to(padding.into(), padding.into());
    let layout = pangocairo::functions::create_layout(&cr);
    layout.context().set_round_glyph_positions(false);
    layout.set_font_description(Some(&font));
    layout.set_text(text);

    cr.set_source_rgb(1., 1., 1.);
    pangocairo::functions::show_layout(&cr, &layout);
    drop(cr);

    let data = surface.take_data()?;
    let buffer = TextureBuffer::from_memory(
        renderer,
        &data,
        Fourcc::Argb8888,
        (width, height),
        false,
        scale,
        Transform::Normal,
        Vec::new(),
    )?;

    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use niri_config::Config;

    use super::*;

    fn label_at(time: Duration) -> ViewOutputLabel {
        ViewOutputLabel::new(
            Clock::with_time(time),
            Rc::new(RefCell::new(Config::default())),
        )
    }

    fn finish_animation(label: &mut ViewOutputLabel) {
        let mut clock = label.clock.clone();
        clock.set_complete_instantly(true);
        label.advance_animations();
        clock.set_complete_instantly(false);
    }

    #[test]
    fn new_is_hidden() {
        let label = label_at(Duration::ZERO);

        assert!(!label.are_animations_ongoing());
        assert!(matches!(label.state, State::Hidden));
    }

    #[test]
    fn show_starts_showing() {
        let mut label = label_at(Duration::ZERO);

        label.show("Viewing: steam".to_owned());

        assert!(label.are_animations_ongoing());
        assert!(matches!(label.state, State::Showing(_)));
        assert_eq!(label.text, "Viewing: steam");
    }

    #[test]
    fn show_animation_completes_into_shown_then_hides_after_duration() {
        let mut label = label_at(Duration::ZERO);
        label.show("Viewing: steam".to_owned());

        finish_animation(&mut label);
        assert!(matches!(label.state, State::Shown(_)));
        assert!(label.are_animations_ongoing());

        // Not enough time has passed yet; still shown.
        label
            .clock
            .set_unadjusted(VIEW_OUTPUT_LABEL_DURATION - Duration::from_millis(1));
        label.advance_animations();
        assert!(matches!(label.state, State::Shown(_)));

        // Once the duration elapses, it starts hiding.
        label
            .clock
            .set_unadjusted(VIEW_OUTPUT_LABEL_DURATION + Duration::from_millis(1));
        label.advance_animations();
        assert!(matches!(label.state, State::Hiding(_)));

        finish_animation(&mut label);
        assert!(matches!(label.state, State::Hidden));
        assert!(!label.are_animations_ongoing());
    }

    #[test]
    fn show_while_shown_restarts_with_new_text() {
        let mut label = label_at(Duration::ZERO);
        label.show("Viewing: steam".to_owned());
        finish_animation(&mut label);
        assert!(matches!(label.state, State::Shown(_)));

        label.show("Stopped viewing steam: output removed".to_owned());

        assert!(matches!(label.state, State::Showing(_)));
        assert_eq!(label.text, "Stopped viewing steam: output removed");
    }

    #[test]
    fn hide_moves_to_hiding() {
        let mut label = label_at(Duration::ZERO);
        label.show("Viewing: steam".to_owned());
        finish_animation(&mut label);

        label.hide();

        assert!(matches!(label.state, State::Hiding(_)));
    }

    #[test]
    fn hide_on_hidden_is_a_no_op() {
        let mut label = label_at(Duration::ZERO);

        label.hide();

        assert!(matches!(label.state, State::Hidden));
    }
}

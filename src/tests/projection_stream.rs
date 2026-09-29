//! Rendering-side guarantees for projections: the source output's own
//! rendering must never be affected by, or leak into, a projection (FR-016,
//! FR-017, FR-018).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use smithay::backend::input::InputTime;
use smithay::backend::renderer::element::Element as _;
use smithay::input::pointer::MotionEvent;
use smithay::output::Output;
use smithay::utils::{Physical, Rectangle, Scale, SERIAL_COUNTER};

use crate::niri::{OutputRenderElements, RedrawState};
use crate::projection::{ProjectionKind, ViewOrigin, Viewing};
use crate::render_helpers::{RenderCtx, RenderTarget};
use crate::tests::fixture::Fixture;

fn set_up_with_source() -> Fixture {
    let mut f = Fixture::new();
    f.niri_state().backend.headless().add_renderer().unwrap();
    f.add_output(1, (1920, 1080));

    let state = f.niri_state();
    state
        .backend
        .headless()
        .create_virtual_output(&mut state.niri, 1280, 800, 60, Some("steam".to_string()))
        .unwrap();

    f
}

fn output_named(f: &mut Fixture, name: &str) -> Output {
    f.niri()
        .layout
        .outputs()
        .find(|o| o.name() == name)
        .unwrap()
        .clone()
}

/// Maps a client window on `output` so the source has real content to render.
fn map_window_on(f: &mut Fixture, output: &Output, w: u16, h: u16) {
    let id = f.add_client();
    f.niri().layout.focus_output(output);

    let window = f.client(id).create_window();
    let surface = window.surface.clone();
    window.commit();
    f.roundtrip(id);

    let window = f.client(id).window(&surface);
    window.attach_new_buffer();
    window.set_size(w, h);
    window.ack_last_and_commit();
    f.double_roundtrip(id);
}

/// Renders `output` (real renderer, no pointer) and returns each element's
/// geometry at `output`'s own scale, kind-agnostically (i.e. regardless of
/// which `OutputRenderElements` variant produced it).
fn source_geometries(f: &mut Fixture, output: &Output) -> Vec<Rectangle<i32, Physical>> {
    f.niri().update_render_elements(None);
    let scale = Scale::from(output.current_scale().fractional_scale());
    let state = f.niri_state();
    let niri = &state.niri;
    state
        .backend
        .headless()
        .with_primary_renderer(|renderer| {
            let ctx = RenderCtx {
                renderer,
                target: RenderTarget::Output,
                xray: None,
            };
            niri.render_to_vec(ctx, output, false)
                .iter()
                .map(|e| e.geometry(scale))
                .collect()
        })
        .unwrap()
}

fn render_has_pointer(f: &mut Fixture, output: &Output) -> bool {
    f.niri().update_render_elements(None);
    let state = f.niri_state();
    let niri = &state.niri;
    state
        .backend
        .headless()
        .with_primary_renderer(|renderer| {
            let ctx = RenderCtx {
                renderer,
                target: RenderTarget::Output,
                xray: None,
            };
            niri.render_to_vec(ctx, output, true)
                .iter()
                .any(|e| matches!(e, OutputRenderElements::Pointer(_)))
        })
        .unwrap()
}

// T047: rendering the source must produce the same elements whether or not
// a projection of it exists.
//
// Opening the overview also puts the source's OWN monitor into overview mode
// (its zoom and workspace layout change), so comparing the pre-overview
// baseline directly against "overview open" would fail for a reason that has
// nothing to do with projections. Instead, for each state (overview open;
// viewing set), we render once with the real projection list, then again
// with `projection_state.projections` cleared but everything else identical,
// and compare those two. That isolates the one variable this guarantee is
// actually about: whether a projection existing changes the source's
// elements. View mode is additionally compared against the plain baseline,
// since (unlike the overview) it never touches the source's own monitor
// state, so leak-free implies those two must match exactly.
#[test]
fn egl_source_render_is_unaffected_by_overview_and_view_projections() {
    let mut f = set_up_with_source();
    let steam = output_named(&mut f, "steam");
    map_window_on(&mut f, &steam, 300, 200);
    // Settle the new window's open animation so the baseline reflects its
    // final geometry, not a mid-animation frame.
    f.niri_complete_animations();
    let viewer = f.niri_output(1);

    assert!(f.niri().projection_state.projections.is_empty());
    let baseline = source_geometries(&mut f, &steam);
    assert!(!baseline.is_empty(), "expected the mapped window to render");

    // (b) Overview open: an Overview projection now exists for steam.
    f.niri().layout.toggle_overview();
    f.niri_complete_animations();
    assert!(f.niri().layout.is_overview_open());
    assert!(f
        .niri()
        .projection_state
        .projections
        .iter()
        .any(|p| p.source == "steam" && p.kind == ProjectionKind::Overview));

    let with_overview_projection = source_geometries(&mut f, &steam);
    let saved = std::mem::take(&mut f.niri().projection_state.projections);
    let overview_projection_cleared = source_geometries(&mut f, &steam);
    f.niri().projection_state.projections = saved;

    assert_eq!(
        with_overview_projection, overview_projection_cleared,
        "the source's own overview-mode rendering must not depend on whether \
         an Overview projection of it exists"
    );

    f.niri().layout.toggle_overview();
    f.niri_complete_animations();
    assert!(!f.niri().layout.is_overview_open());
    assert!(f.niri().projection_state.projections.is_empty());

    // (c) View mode: viewing set, overview closed -> a View projection exists.
    f.niri().projection_state.viewing = Some(Viewing {
        viewer: viewer.name(),
        source: "steam".to_string(),
        origin: ViewOrigin::Command,
    });
    f.niri().rebuild_projections();
    assert_eq!(f.niri().projection_state.projections.len(), 1);
    assert_eq!(
        f.niri().projection_state.projections[0].kind,
        ProjectionKind::View
    );

    let with_view_projection = source_geometries(&mut f, &steam);
    let saved = std::mem::take(&mut f.niri().projection_state.projections);
    let view_projection_cleared = source_geometries(&mut f, &steam);
    f.niri().projection_state.projections = saved;

    assert_eq!(
        with_view_projection, view_projection_cleared,
        "the source's rendering must not depend on whether a View projection \
         of it exists"
    );
    assert_eq!(
        baseline, view_projection_cleared,
        "being viewed must not otherwise change the source's own rendering"
    );
}

// T048: FR-017. A physical cursor sitting on the viewer, inside a View
// projection's region, must not show up when the SOURCE is rendered
// (only the viewer, which the cursor is actually on, draws it).
#[test]
fn egl_pointer_on_the_viewer_inside_a_view_projection_is_not_drawn_on_the_source() {
    let mut f = set_up_with_source();
    let steam = output_named(&mut f, "steam");
    let viewer = f.niri_output(1);

    f.niri().projection_state.viewing = Some(Viewing {
        viewer: viewer.name(),
        source: "steam".to_string(),
        origin: ViewOrigin::Command,
    });
    f.niri().rebuild_projections();

    let projection = f.niri().projection_state.projections[0].clone();
    assert_eq!(projection.kind, ProjectionKind::View);

    let region = projection.region;
    let region_center = region.loc + region.size.downscale(2.).to_point();
    let viewer_origin = f
        .niri()
        .global_space
        .output_geometry(&viewer)
        .unwrap()
        .loc
        .to_f64();
    let global_pos = region_center + viewer_origin;

    let pointer = f.niri().seat.get_pointer().unwrap();
    let state = f.niri_state();
    pointer.motion(
        state,
        None,
        &MotionEvent {
            location: global_pos,
            serial: SERIAL_COUNTER.next_serial(),
            time: InputTime::from_millis(0),
        },
    );
    pointer.frame(state);

    // Sanity: the point really does resolve onto the source through the
    // projection, and not onto the viewer's own content.
    let (under, _) = f.niri().output_under(global_pos).unwrap();
    assert_eq!(under.name(), "steam");

    assert!(
        !render_has_pointer(&mut f, &steam),
        "source rendered a pointer element while being viewed through a View projection"
    );
    assert!(
        render_has_pointer(&mut f, &viewer),
        "viewer did not render a pointer element under the real cursor position"
    );
}

// A streaming client moves the seat pointer onto the virtual output itself, and
// the stream must show that cursor: the pointer is drawn on a virtual output
// whenever it is physically over it.
#[test]
fn egl_pointer_physically_on_the_virtual_output_is_drawn_on_it() {
    let mut f = set_up_with_source();
    let steam = output_named(&mut f, "steam");

    let steam_geo = f.niri().global_space.output_geometry(&steam).unwrap();
    let global_pos = steam_geo.loc.to_f64() + steam_geo.size.to_f64().downscale(2.).to_point();

    let pointer = f.niri().seat.get_pointer().unwrap();
    let state = f.niri_state();
    pointer.motion(
        state,
        None,
        &MotionEvent {
            location: global_pos,
            serial: SERIAL_COUNTER.next_serial(),
            time: InputTime::from_millis(0),
        },
    );
    pointer.frame(state);

    assert!(
        render_has_pointer(&mut f, &steam),
        "a pointer physically on the virtual output was not drawn on it (remote cursor lost)"
    );
}

// T047 (frame clock / redraw isolation): rendering a viewer never advances
// or resets the source's redraw state or frame clock.
//
// `Niri::render`/`render_to_vec` take `&self`, so this is structurally
// guaranteed at compile time: there is no `&mut self` access path from a
// render call into `output_state`. `frame_callback_sequence` and
// `redraw_state` are the only observable fields on `OutputState` for this
// (the frame clock's `last_presentation_time` is private); this test pins
// that observation down so a future refactor that adds mutation to the
// render path would fail it immediately.
/// `RedrawState` has no `PartialEq` (some variants hold a `RegistrationToken`
/// tied to the event loop); this names the variant kind so two snapshots can
/// be compared without caring about that payload.
fn redraw_state_kind(state: &RedrawState) -> &'static str {
    match state {
        RedrawState::Idle => "idle",
        RedrawState::Queued => "queued",
        RedrawState::WaitingForVBlank { .. } => "waiting_for_vblank",
        RedrawState::WaitingForEstimatedVBlank(_) => "waiting_for_estimated_vblank",
        RedrawState::WaitingForEstimatedVBlankAndQueued(_) => {
            "waiting_for_estimated_vblank_and_queued"
        }
    }
}

#[test]
fn egl_rendering_the_viewer_does_not_touch_the_source_redraw_state() {
    let mut f = set_up_with_source();
    let steam = output_named(&mut f, "steam");
    let viewer = f.niri_output(1);

    let before_sequence = f.niri().output_state[&steam].frame_callback_sequence;
    let before_redraw_state = redraw_state_kind(&f.niri().output_state[&steam].redraw_state);

    for _ in 0..5 {
        let _ = source_geometries(&mut f, &viewer);
    }

    assert_eq!(
        f.niri().output_state[&steam].frame_callback_sequence,
        before_sequence
    );
    assert_eq!(
        redraw_state_kind(&f.niri().output_state[&steam].redraw_state),
        before_redraw_state
    );
}

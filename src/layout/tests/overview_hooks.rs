use super::*;

// Outputs are 1280x720 at scale 1 and the default overview zoom is 0.5, so a fully open
// overview shows 640x360 workspaces.
const OUTPUT_W: f64 = 1280.;

fn overview_with_workspaces(named: usize) -> Layout<TestWindow> {
    let mut ops = vec![Op::AddOutput(1)];
    for ws_name in 1..=named {
        ops.push(Op::AddNamedWorkspace {
            ws_name,
            output_names: vec![1],
            layout_config: None,
        });
    }
    ops.extend([
        Op::FocusWorkspace(0),
        Op::ToggleOverview,
        Op::CompleteAnimations,
    ]);
    check_ops(ops)
}

fn output(layout: &Layout<TestWindow>) -> Output {
    layout.outputs().next().unwrap().clone()
}

fn monitor(layout: &Layout<TestWindow>) -> &Monitor<TestWindow> {
    layout.monitors().next().unwrap()
}

fn monitor_mut(layout: &mut Layout<TestWindow>) -> &mut Monitor<TestWindow> {
    layout.monitors_mut().next().unwrap()
}

fn render_geo(layout: &Layout<TestWindow>) -> Vec<Rectangle<f64, Logical>> {
    monitor(layout).workspaces_render_geo().collect()
}

fn set_inset(layout: &mut Layout<TestWindow>, inset: f64) {
    let output = output(layout);
    layout.set_overview_right_inset(&output, inset);
}

#[test]
fn overview_right_inset_zero_keeps_geometry() {
    let mut layout = overview_with_workspaces(2);
    let before = render_geo(&layout);
    assert_eq!(before[0].loc.x, (OUTPUT_W - 640.) / 2.);

    set_inset(&mut layout, 0.);
    assert_eq!(render_geo(&layout), before);
}

#[test]
fn overview_right_inset_centres_strip_in_remaining_width() {
    let mut layout = overview_with_workspaces(2);
    set_inset(&mut layout, 256.);

    for geo in render_geo(&layout) {
        assert_eq!(geo.loc.x, (OUTPUT_W - 256. - 640.) / 2.);
        assert_eq!(geo.size.w, 640.);
    }
    layout.verify_invariants();
}

#[test]
fn overview_right_inset_follows_overview_progress() {
    let mut layout = overview_with_workspaces(0);
    set_inset(&mut layout, 256.);

    let half_open = OverviewProgress::Gesture(OverviewGesture {
        tracker: SwipeTracker::new(),
        start: 0.,
        value: 0.5,
    });
    monitor_mut(&mut layout).set_overview_progress(Some(&half_open));

    // Zoom 0.75 at half progress, so 960 wide, and half the inset reserved.
    let geo = render_geo(&layout)[0];
    assert_eq!(geo.size.w, 960.);
    assert_eq!(geo.loc.x, (OUTPUT_W - 128. - 960.) / 2.);
}

#[test]
fn overview_right_inset_has_no_effect_with_the_overview_closed() {
    let mut layout = check_ops([Op::AddOutput(1)]);
    set_inset(&mut layout, 256.);

    let geo = render_geo(&layout)[0];
    assert_eq!(geo.loc.x, 0.);
    let mon = monitor(&layout);
    assert!(mon
        .workspace_under(Point::from((OUTPUT_W - 1., 10.)))
        .is_some());
}

#[test]
fn overview_right_inset_invalid_values_count_as_zero() {
    let mut layout = overview_with_workspaces(2);
    let expected = render_geo(&layout);

    for inset in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.] {
        set_inset(&mut layout, 256.);
        set_inset(&mut layout, inset);
        assert_eq!(render_geo(&layout), expected, "inset {inset}");
    }
}

#[test]
fn overview_right_inset_limits_workspace_under() {
    let mut layout = overview_with_workspaces(2);
    set_inset(&mut layout, 256.);

    let mon = monitor(&layout);
    let y = 360.;
    assert!(mon
        .workspace_under(Point::from((OUTPUT_W - 256. - 1., y)))
        .is_some());
    assert!(mon
        .workspace_under(Point::from((OUTPUT_W - 256., y)))
        .is_none());
    assert!(mon
        .workspace_under(Point::from((OUTPUT_W - 1., y)))
        .is_none());
}

#[test]
fn overview_right_inset_shrinks_the_cull_rect() {
    let mut layout = overview_with_workspaces(2);
    let full = Rectangle::from_size(Size::from((OUTPUT_W, 720.)));
    assert_eq!(monitor(&layout).overview_cull_rect(), full);

    set_inset(&mut layout, 256.);
    let shrunk = Rectangle::from_size(Size::from((OUTPUT_W - 256., 720.)));
    assert_eq!(monitor(&layout).overview_cull_rect(), shrunk);

    // Wider than the output: the rect collapses instead of turning negative.
    set_inset(&mut layout, 2. * OUTPUT_W);
    assert_eq!(monitor(&layout).overview_cull_rect().size.w, 0.);
}

fn dnd_scrolls_at(inset: f64, x: f64) -> bool {
    let mut layout = overview_with_workspaces(2);
    set_inset(&mut layout, inset);
    let mon = monitor_mut(&mut layout);
    mon.dnd_scroll_gesture_begin();
    mon.dnd_scroll_gesture_scroll(Point::from((x, 1.)), 1.)
}

#[test]
fn overview_right_inset_moves_the_dnd_scroll_strip() {
    // Without an inset the 640 wide strip spans 320..960; with 256 reserved, 192..832.
    assert!(dnd_scrolls_at(0., 900.));
    assert!(!dnd_scrolls_at(256., 900.));
    assert!(!dnd_scrolls_at(0., 200.));
    assert!(dnd_scrolls_at(256., 200.));
}

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
    assert_eq!(monitor(&layout).overview_cull_rect(), Some(full));

    set_inset(&mut layout, 256.);
    let shrunk = Rectangle::from_size(Size::from((OUTPUT_W - 256., 720.)));
    assert_eq!(monitor(&layout).overview_cull_rect(), Some(shrunk));

    // Wider than the output: the rect collapses instead of turning negative.
    set_inset(&mut layout, 2. * OUTPUT_W);
    assert_eq!(monitor(&layout).overview_cull_rect().unwrap().size.w, 0.);
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

// Three named workspaces plus the empty last one. With the first one active, the open overview
// shows workspaces 0 and 1; workspace 2 starts below the output.
fn overview_with_offscreen_workspaces(reachable: bool) -> Layout<TestWindow> {
    let mut layout = overview_with_workspaces(3);
    let output = output(&layout);
    layout.set_overview_offscreen_reachable(&output, reachable);
    layout
}

fn centre(geo: Rectangle<f64, Logical>) -> Point<f64, Logical> {
    geo.loc + geo.size.downscale(2.).to_point()
}

#[test]
fn overview_offscreen_reachable_renders_every_workspace() {
    let layout = overview_with_offscreen_workspaces(false);
    assert_eq!(monitor(&layout).workspaces_with_render_geo().count(), 2);
    assert_eq!(monitor(&layout).workspaces_with_render_geo_idx().count(), 2);

    let layout = overview_with_offscreen_workspaces(true);
    let mon = monitor(&layout);
    assert_eq!(mon.workspaces_with_render_geo().count(), 4);
    assert_eq!(mon.workspaces_with_render_geo_idx().count(), 4);
    assert_eq!(mon.workspaces_with_render_geo_cull(true).count(), 4);
}

#[test]
fn overview_offscreen_reachable_hit_tests_offscreen_workspaces() {
    let layout = overview_with_offscreen_workspaces(false);
    let geo = render_geo(&layout);
    assert!(geo[3].loc.y > 720.);
    assert!(monitor(&layout).workspace_under(centre(geo[3])).is_none());

    let layout = overview_with_offscreen_workspaces(true);
    let mon = monitor(&layout);
    let (ws, ws_geo) = mon.workspace_under(centre(geo[3])).unwrap();
    assert_eq!(ws.id(), mon.workspaces[3].id());
    assert_eq!(ws_geo, geo[3]);
}

#[test]
fn overview_offscreen_reachable_insert_position_reaches_offscreen_workspaces() {
    let layout = overview_with_offscreen_workspaces(false);
    let geo = render_geo(&layout);
    let past_last = centre(geo[4]);
    let (insert_ws, _) = monitor(&layout).insert_position(past_last);
    assert_eq!(insert_ws, InsertWorkspace::NewAt(2));

    let layout = overview_with_offscreen_workspaces(true);
    let mon = monitor(&layout);
    let (insert_ws, insert_geo) = mon.insert_position(centre(geo[2]));
    assert_eq!(insert_ws, InsertWorkspace::Existing(mon.workspaces[2].id()));
    assert_eq!(insert_geo, geo[2]);

    let (insert_ws, _) = mon.insert_position(past_last);
    assert_eq!(insert_ws, InsertWorkspace::NewAt(4));
}

#[test]
fn overview_offscreen_reachable_needs_the_overview() {
    let mut layout = overview_with_offscreen_workspaces(true);
    check_ops_on_layout(&mut layout, [Op::ToggleOverview, Op::CompleteAnimations]);

    assert_eq!(monitor(&layout).workspaces_with_render_geo().count(), 1);
}

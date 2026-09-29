//! What the compositor sends on the IPC event stream, read through a socketless subscriber.

use std::collections::HashMap;

use async_channel::Receiver;
use niri_config::{Action, OutputName};
use niri_ipc::{Event, Output as IpcOutput, ViewOutputState};
use smithay::output::Output;

use super::fixture::Fixture;
use super::input;
use crate::layout::workspace::WorkspaceId;
use crate::projection::ViewOrigin;

fn set_up(sources: &[&str]) -> Fixture {
    let mut f = Fixture::new();
    f.add_output(1, (1920, 1080));
    for name in sources {
        create(&mut f, name);
    }
    f
}

fn create(f: &mut Fixture, name: &str) {
    let state = f.niri_state();
    state
        .backend
        .headless()
        .create_virtual_output(&mut state.niri, 1280, 800, 60, Some(name.to_string()))
        .unwrap();
}

fn output_named(f: &mut Fixture, name: &str) -> Output {
    f.niri()
        .layout
        .outputs()
        .find(|o| o.name() == name)
        .unwrap()
        .clone()
}

/// What the event loop runs after dispatching each batch of events.
fn refresh(f: &mut Fixture) {
    f.niri_state().refresh_and_flush_clients();
}

fn drain(events: &Receiver<Event>) -> Vec<Event> {
    std::iter::from_fn(|| events.try_recv().ok()).collect()
}

/// A subscriber that has already read the initial burst.
fn subscribe(f: &mut Fixture) -> Receiver<Event> {
    refresh(f);
    let events = f.niri().ipc_server.as_ref().unwrap().subscribe();
    drain(&events);
    events
}

fn view_events(f: &mut Fixture, events: &Receiver<Event>) -> Vec<ViewOutputState> {
    refresh(f);
    drain(events)
        .into_iter()
        .filter_map(|event| match event {
            Event::ViewOutputChanged { state } => Some(state),
            _ => None,
        })
        .collect()
}

fn outputs_events(f: &mut Fixture, events: &Receiver<Event>) -> Vec<Vec<(String, bool)>> {
    refresh(f);
    drain(events)
        .into_iter()
        .filter_map(|event| match event {
            Event::OutputsChanged { outputs } => Some(summary(&outputs)),
            _ => None,
        })
        .collect()
}

fn viewing(source: &str) -> ViewOutputState {
    ViewOutputState::Viewing {
        viewer: "headless-1".to_string(),
        source: source.to_string(),
    }
}

/// Names and on/off state, sorted by name.
fn summary(outputs: &HashMap<String, IpcOutput>) -> Vec<(String, bool)> {
    let mut summary: Vec<_> = outputs
        .iter()
        .map(|(name, output)| {
            assert_eq!(name, &output.name);
            (name.clone(), output.logical.is_some())
        })
        .collect();
    summary.sort();
    summary
}

fn on_off(entries: &[(&str, bool)]) -> Vec<(String, bool)> {
    entries.iter().map(|(n, on)| (n.to_string(), *on)).collect()
}

/// Opens the overview and clicks the workspace of `source` on the viewer.
fn view_from_the_overview(f: &mut Fixture, source: &str) {
    let viewer = f.niri_output(1);
    let source = output_named(f, source);
    f.niri().layout.toggle_overview();
    f.niri_complete_animations();
    let ws_id: WorkspaceId = f
        .niri()
        .layout
        .workspaces()
        .find(|(mon, _, _)| mon.is_some_and(|m| m.output() == &source))
        .map(|(_, _, ws)| ws.id())
        .unwrap();

    f.niri()
        .activate_overview_workspace(&source, ws_id, &viewer);
    f.niri_complete_animations();
}

#[test]
fn view_output_request_sends_one_viewing_event() {
    let mut f = set_up(&["steam"]);
    let events = subscribe(&mut f);

    f.niri_state().view_output(Some("steam")).unwrap();

    assert_eq!(view_events(&mut f, &events), [viewing("steam")]);
    assert_eq!(view_events(&mut f, &events), []);
}

#[test]
fn view_output_bind_sends_one_event_to_start_and_one_to_stop() {
    let mut f = set_up(&["steam"]);
    let events = subscribe(&mut f);

    f.niri_state()
        .do_action(Action::ViewOutput(Some("steam".to_string())), false);
    assert_eq!(view_events(&mut f, &events), [viewing("steam")]);

    f.niri_state().do_action(Action::ViewOutput(None), false);
    assert_eq!(view_events(&mut f, &events), [ViewOutputState::NotViewing]);
}

#[test]
fn clicking_a_source_workspace_in_the_overview_sends_one_viewing_event() {
    let mut f = set_up(&["steam"]);
    let events = subscribe(&mut f);

    view_from_the_overview(&mut f, "steam");

    assert_eq!(
        f.niri().projection_state.viewing.as_ref().map(|v| v.origin),
        Some(ViewOrigin::Overview)
    );
    assert_eq!(view_events(&mut f, &events), [viewing("steam")]);
}

#[test]
fn view_output_without_a_name_sends_one_not_viewing_event() {
    let mut f = set_up(&["steam"]);
    f.niri_state().view_output(Some("steam")).unwrap();
    let events = subscribe(&mut f);

    f.niri_state().view_output(None).unwrap();

    assert_eq!(view_events(&mut f, &events), [ViewOutputState::NotViewing]);
}

#[test]
fn view_output_without_a_name_while_not_viewing_sends_nothing() {
    let mut f = set_up(&["steam"]);
    let events = subscribe(&mut f);

    f.niri_state().view_output(None).unwrap();

    assert_eq!(view_events(&mut f, &events), []);
}

#[test]
fn escape_in_a_view_from_the_overview_sends_one_not_viewing_event() {
    let mut f = set_up(&["steam"]);
    view_from_the_overview(&mut f, "steam");
    let events = subscribe(&mut f);

    input::key(&mut f, "ESC", true);
    refresh(&mut f);
    input::key(&mut f, "ESC", false);

    assert_eq!(f.niri().projection_state.viewing, None);
    assert_eq!(view_events(&mut f, &events), [ViewOutputState::NotViewing]);
}

#[test]
fn the_viewer_becoming_active_sends_one_not_viewing_event() {
    let mut f = set_up(&["steam"]);
    f.niri_state().view_output(Some("steam")).unwrap();
    let events = subscribe(&mut f);

    let viewer = f.niri_output(1);
    f.niri().layout.focus_output(&viewer);

    assert_eq!(view_events(&mut f, &events), [ViewOutputState::NotViewing]);
}

#[test]
fn turning_the_source_off_sends_one_not_viewing_event() {
    let mut f = set_up(&["steam"]);
    f.niri_state().view_output(Some("steam")).unwrap();
    let events = subscribe(&mut f);

    // Turning a virtual output off takes it out of the layout while the backend keeps it.
    let steam = output_named(&mut f, "steam");
    f.niri().remove_output(&steam);

    assert_eq!(view_events(&mut f, &events), [ViewOutputState::NotViewing]);
}

#[test]
fn removing_the_source_sends_one_not_viewing_event() {
    let mut f = set_up(&["steam"]);
    f.niri_state().view_output(Some("steam")).unwrap();
    let events = subscribe(&mut f);

    let state = f.niri_state();
    state
        .backend
        .headless()
        .remove_virtual_output(&mut state.niri, "steam")
        .unwrap();

    assert_eq!(view_events(&mut f, &events), [ViewOutputState::NotViewing]);
}

#[test]
fn removing_the_viewer_sends_one_not_viewing_event() {
    let mut f = set_up(&["steam"]);
    f.niri_state().view_output(Some("steam")).unwrap();
    let events = subscribe(&mut f);

    let viewer = f.niri_output(1);
    f.niri().remove_output(&viewer);

    assert_eq!(view_events(&mut f, &events), [ViewOutputState::NotViewing]);
}

#[test]
fn switching_the_viewed_source_sends_one_viewing_event() {
    let mut f = set_up(&["steam", "aux"]);
    f.niri_state().view_output(Some("steam")).unwrap();
    let events = subscribe(&mut f);

    f.niri_state().view_output(Some("aux")).unwrap();

    assert_eq!(view_events(&mut f, &events), [viewing("aux")]);
}

#[test]
fn creating_a_virtual_output_sends_one_outputs_event() {
    let mut f = set_up(&[]);
    let events = subscribe(&mut f);

    create(&mut f, "steam");

    let expected = on_off(&[("headless-1", true), ("steam", true)]);
    assert_eq!(outputs_events(&mut f, &events), [expected]);
    assert_eq!(outputs_events(&mut f, &events), Vec::<Vec<_>>::new());
}

#[test]
fn removing_a_virtual_output_sends_one_outputs_event() {
    let mut f = set_up(&["steam"]);
    let events = subscribe(&mut f);

    let state = f.niri_state();
    state
        .backend
        .headless()
        .remove_virtual_output(&mut state.niri, "steam")
        .unwrap();

    let expected = on_off(&[("headless-1", true)]);
    assert_eq!(outputs_events(&mut f, &events), [expected]);
}

#[test]
fn turning_an_output_off_and_on_sends_one_outputs_event_each() {
    let mut f = set_up(&["steam"]);
    let events = subscribe(&mut f);
    let steam = output_named(&mut f, "steam");

    f.niri().remove_output(&steam);
    let expected = on_off(&[("headless-1", true), ("steam", false)]);
    assert_eq!(outputs_events(&mut f, &events), [expected]);

    // Turning back on adds a fresh Output with the same identity, as the TTY backend does.
    let rebuilt = Output::new(steam.name(), steam.physical_properties());
    let mode = steam.current_mode().unwrap();
    rebuilt.change_current_state(Some(mode), None, None, None);
    rebuilt.set_preferred(mode);
    let name = steam.user_data().get::<OutputName>().unwrap().clone();
    rebuilt.user_data().insert_if_missing(|| name);
    f.niri().add_output(rebuilt, None, false);

    let expected = on_off(&[("headless-1", true), ("steam", true)]);
    assert_eq!(outputs_events(&mut f, &events), [expected]);
}

#[test]
fn a_mode_change_alone_sends_no_outputs_event() {
    let mut f = set_up(&["steam"]);
    let events = subscribe(&mut f);

    let state = f.niri_state();
    for output in state.backend.ipc_outputs().lock().unwrap().values_mut() {
        if output.name == "steam" {
            let mut mode = output.modes[0];
            mode.width = 1920;
            mode.height = 1200;
            output.modes.push(mode);
            output.current_mode = Some(1);
        }
    }
    state.niri.ipc_outputs_changed = true;

    assert_eq!(outputs_events(&mut f, &events), Vec::<Vec<_>>::new());
}

#[test]
fn a_new_subscriber_gets_the_current_view_mode_and_outputs() {
    let mut f = set_up(&["steam"]);
    f.niri_state().view_output(Some("steam")).unwrap();
    refresh(&mut f);

    let initial = drain(&f.niri().ipc_server.as_ref().unwrap().subscribe());

    let view: Vec<_> = initial
        .iter()
        .filter_map(|event| match event {
            Event::ViewOutputChanged { state } => Some(state.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(view, [viewing("steam")]);
    let outputs: Vec<_> = initial
        .iter()
        .filter_map(|event| match event {
            Event::OutputsChanged { outputs } => Some(summary(outputs)),
            _ => None,
        })
        .collect();
    assert_eq!(outputs, [on_off(&[("headless-1", true), ("steam", true)])]);
}

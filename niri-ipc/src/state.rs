//! Helpers for keeping track of the event stream state.
//!
//! 1. Create an [`EventStreamState`] using `Default::default()`, or any individual state part if
//!    you only care about part of the state.
//! 2. Connect to the niri socket and request an event stream.
//! 3. Pass every [`Event`] to [`EventStreamStatePart::apply`] on your state.
//! 4. Read the fields of the state as needed.

use std::collections::hash_map::Entry;
use std::collections::HashMap;

use crate::{Cast, Event, KeyboardLayouts, Output, ViewOutputState, Window, Workspace};

/// Part of the state communicated via the event stream.
pub trait EventStreamStatePart {
    /// Returns a sequence of events that replicates this state from default initialization.
    fn replicate(&self) -> Vec<Event>;

    /// Applies the event to this state.
    ///
    /// Returns `None` after applying the event, and `Some(event)` if the event is ignored by this
    /// part of the state.
    fn apply(&mut self, event: Event) -> Option<Event>;
}

/// The full state communicated over the event stream.
///
/// Different parts of the state are not guaranteed to be consistent across every single event
/// sent by niri. For example, you may receive the first [`Event::WindowOpenedOrChanged`] for a
/// just-opened window *after* an [`Event::WorkspaceActiveWindowChanged`] for that window. Between
/// these two events, the workspace active window id refers to a window that does not yet exist in
/// the windows state part.
#[derive(Debug, Default)]
pub struct EventStreamState {
    /// State of workspaces.
    pub workspaces: WorkspacesState,

    /// State of workspaces.
    pub windows: WindowsState,

    /// State of the keyboard layouts.
    pub keyboard_layouts: KeyboardLayoutsState,

    /// State of the overview.
    pub overview: OverviewState,

    /// State of the config.
    pub config: ConfigState,

    /// State of screencasts.
    pub casts: CastsState,

    /// State of view mode.
    pub view_output: ViewOutputEventState,

    /// State of the outputs.
    pub outputs: OutputsState,
}

/// The workspaces state communicated over the event stream.
#[derive(Debug, Default)]
pub struct WorkspacesState {
    /// Map from a workspace id to the workspace.
    pub workspaces: HashMap<u64, Workspace>,
}

/// The windows state communicated over the event stream.
#[derive(Debug, Default)]
pub struct WindowsState {
    /// Map from a window id to the window.
    pub windows: HashMap<u64, Window>,
}

/// The keyboard layout state communicated over the event stream.
#[derive(Debug, Default)]
pub struct KeyboardLayoutsState {
    /// Configured keyboard layouts.
    pub keyboard_layouts: Option<KeyboardLayouts>,
}

/// The overview state communicated over the event stream.
#[derive(Debug, Default)]
pub struct OverviewState {
    /// Whether the overview is currently open.
    pub is_open: bool,
}

/// The config state communicated over the event stream.
#[derive(Debug, Default)]
pub struct ConfigState {
    /// Whether the last config load attempt had failed.
    pub failed: bool,
}

/// The casts state communicated over the event stream.
#[derive(Debug, Default)]
pub struct CastsState {
    /// Map from a stream id to the screencast.
    pub casts: HashMap<u64, Cast>,
}

/// The view mode state communicated over the event stream.
#[derive(Debug)]
pub struct ViewOutputEventState {
    /// Whether a physical output is viewing a virtual output, and which.
    pub state: ViewOutputState,
}

impl Default for ViewOutputEventState {
    fn default() -> Self {
        Self {
            state: ViewOutputState::NotViewing,
        }
    }
}

/// The outputs state communicated over the event stream.
#[derive(Debug, Default)]
pub struct OutputsState {
    /// Map from an output name to the output.
    pub outputs: HashMap<String, Output>,
}

impl EventStreamStatePart for EventStreamState {
    fn replicate(&self) -> Vec<Event> {
        let mut events = Vec::new();
        events.extend(self.workspaces.replicate());
        events.extend(self.windows.replicate());
        events.extend(self.keyboard_layouts.replicate());
        events.extend(self.overview.replicate());
        events.extend(self.config.replicate());
        events.extend(self.casts.replicate());
        events.extend(self.view_output.replicate());
        events.extend(self.outputs.replicate());
        events
    }

    fn apply(&mut self, event: Event) -> Option<Event> {
        let event = self.workspaces.apply(event)?;
        let event = self.windows.apply(event)?;
        let event = self.keyboard_layouts.apply(event)?;
        let event = self.overview.apply(event)?;
        let event = self.config.apply(event)?;
        let event = self.casts.apply(event)?;
        let event = self.view_output.apply(event)?;
        let event = self.outputs.apply(event)?;
        Some(event)
    }
}

impl EventStreamStatePart for WorkspacesState {
    fn replicate(&self) -> Vec<Event> {
        let workspaces = self.workspaces.values().cloned().collect();
        vec![Event::WorkspacesChanged { workspaces }]
    }

    fn apply(&mut self, event: Event) -> Option<Event> {
        match event {
            Event::WorkspacesChanged { workspaces } => {
                self.workspaces = workspaces.into_iter().map(|ws| (ws.id, ws)).collect();
            }
            Event::WorkspaceUrgencyChanged { id, urgent } => {
                for ws in self.workspaces.values_mut() {
                    if ws.id == id {
                        ws.is_urgent = urgent;
                    }
                }
            }
            Event::WorkspaceActivated { id, focused } => {
                let ws = self.workspaces.get(&id);
                let ws = ws.expect("activated workspace was missing from the map");
                let output = ws.output.clone();

                for ws in self.workspaces.values_mut() {
                    let got_activated = ws.id == id;
                    if ws.output == output {
                        ws.is_active = got_activated;
                    }

                    if focused {
                        ws.is_focused = got_activated;
                    }
                }
            }
            Event::WorkspaceActiveWindowChanged {
                workspace_id,
                active_window_id,
            } => {
                let ws = self.workspaces.get_mut(&workspace_id);
                let ws = ws.expect("changed workspace was missing from the map");
                ws.active_window_id = active_window_id;
            }
            event => return Some(event),
        }
        None
    }
}

impl EventStreamStatePart for WindowsState {
    fn replicate(&self) -> Vec<Event> {
        let windows = self.windows.values().cloned().collect();
        vec![Event::WindowsChanged { windows }]
    }

    fn apply(&mut self, event: Event) -> Option<Event> {
        match event {
            Event::WindowsChanged { windows } => {
                self.windows = windows.into_iter().map(|win| (win.id, win)).collect();
            }
            Event::WindowOpenedOrChanged { window } => {
                let (id, is_focused) = match self.windows.entry(window.id) {
                    Entry::Occupied(mut entry) => {
                        let entry = entry.get_mut();
                        *entry = window;
                        (entry.id, entry.is_focused)
                    }
                    Entry::Vacant(entry) => {
                        let entry = entry.insert(window);
                        (entry.id, entry.is_focused)
                    }
                };

                if is_focused {
                    for win in self.windows.values_mut() {
                        if win.id != id {
                            win.is_focused = false;
                        }
                    }
                }
            }
            Event::WindowClosed { id } => {
                let win = self.windows.remove(&id);
                win.expect("closed window was missing from the map");
            }
            Event::WindowFocusChanged { id } => {
                for win in self.windows.values_mut() {
                    win.is_focused = Some(win.id) == id;
                }
            }
            Event::WindowFocusTimestampChanged {
                id,
                focus_timestamp,
            } => {
                for win in self.windows.values_mut() {
                    if win.id == id {
                        win.focus_timestamp = focus_timestamp;
                        break;
                    }
                }
            }
            Event::WindowUrgencyChanged { id, urgent } => {
                for win in self.windows.values_mut() {
                    if win.id == id {
                        win.is_urgent = urgent;
                        break;
                    }
                }
            }
            Event::WindowLayoutsChanged { changes } => {
                for (id, update) in changes {
                    let win = self.windows.get_mut(&id);
                    let win = win.expect("changed window was missing from the map");
                    win.layout = update;
                }
            }
            event => return Some(event),
        }
        None
    }
}

impl EventStreamStatePart for KeyboardLayoutsState {
    fn replicate(&self) -> Vec<Event> {
        if let Some(keyboard_layouts) = self.keyboard_layouts.clone() {
            vec![Event::KeyboardLayoutsChanged { keyboard_layouts }]
        } else {
            vec![]
        }
    }

    fn apply(&mut self, event: Event) -> Option<Event> {
        match event {
            Event::KeyboardLayoutsChanged { keyboard_layouts } => {
                self.keyboard_layouts = Some(keyboard_layouts);
            }
            Event::KeyboardLayoutSwitched { idx } => {
                let kb = self.keyboard_layouts.as_mut();
                let kb = kb.expect("keyboard layouts must be set before a layout can be switched");
                kb.current_idx = idx;
            }
            event => return Some(event),
        }
        None
    }
}

impl EventStreamStatePart for OverviewState {
    fn replicate(&self) -> Vec<Event> {
        vec![Event::OverviewOpenedOrClosed {
            is_open: self.is_open,
        }]
    }

    fn apply(&mut self, event: Event) -> Option<Event> {
        match event {
            Event::OverviewOpenedOrClosed { is_open } => {
                self.is_open = is_open;
            }
            event => return Some(event),
        }
        None
    }
}

impl EventStreamStatePart for ConfigState {
    fn replicate(&self) -> Vec<Event> {
        vec![Event::ConfigLoaded {
            failed: self.failed,
        }]
    }

    fn apply(&mut self, event: Event) -> Option<Event> {
        match event {
            Event::ConfigLoaded { failed } => {
                self.failed = failed;
            }
            event => return Some(event),
        }
        None
    }
}

impl EventStreamStatePart for ViewOutputEventState {
    fn replicate(&self) -> Vec<Event> {
        vec![Event::ViewOutputChanged {
            state: self.state.clone(),
        }]
    }

    fn apply(&mut self, event: Event) -> Option<Event> {
        match event {
            Event::ViewOutputChanged { state } => {
                self.state = state;
            }
            event => return Some(event),
        }
        None
    }
}

impl EventStreamStatePart for OutputsState {
    fn replicate(&self) -> Vec<Event> {
        vec![Event::OutputsChanged {
            outputs: self.outputs.clone(),
        }]
    }

    fn apply(&mut self, event: Event) -> Option<Event> {
        match event {
            Event::OutputsChanged { outputs } => {
                self.outputs = outputs;
            }
            event => return Some(event),
        }
        None
    }
}

impl EventStreamStatePart for CastsState {
    fn replicate(&self) -> Vec<Event> {
        let casts = self.casts.values().cloned().collect();
        vec![Event::CastsChanged { casts }]
    }

    fn apply(&mut self, event: Event) -> Option<Event> {
        match event {
            Event::CastsChanged { casts } => {
                self.casts = casts.into_iter().map(|c| (c.stream_id, c)).collect();
            }
            Event::CastStartedOrChanged { cast } => {
                self.casts.insert(cast.stream_id, cast);
            }
            Event::CastStopped { stream_id } => {
                let cast = self.casts.remove(&stream_id);
                cast.expect("stopped cast was missing from the map");
            }
            event => return Some(event),
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn output(name: &str) -> Output {
        Output {
            name: name.to_string(),
            make: String::new(),
            model: String::new(),
            serial: None,
            physical_size: None,
            modes: vec![],
            current_mode: None,
            is_custom_mode: false,
            vrr_supported: false,
            vrr_enabled: false,
            logical: None,
            max_bpc: None,
        }
    }

    fn viewing() -> ViewOutputState {
        ViewOutputState::Viewing {
            viewer: "DP-2".to_string(),
            source: "steam".to_string(),
        }
    }

    #[test]
    fn view_output_state_replicates_not_viewing_initially() {
        let state = ViewOutputEventState::default();
        let events = state.replicate();
        assert!(matches!(
            events.as_slice(),
            [Event::ViewOutputChanged {
                state: ViewOutputState::NotViewing
            }]
        ));
    }

    #[test]
    fn view_output_state_applies_its_event_and_replicates_it() {
        let mut state = ViewOutputEventState::default();
        let rest = state.apply(Event::ViewOutputChanged { state: viewing() });
        assert!(rest.is_none());
        assert_eq!(state.state, viewing());

        let events = state.replicate();
        assert!(matches!(
            events.as_slice(),
            [Event::ViewOutputChanged { state }] if *state == viewing()
        ));
    }

    #[test]
    fn view_output_state_passes_other_events_on() {
        let mut state = ViewOutputEventState::default();
        let rest = state.apply(Event::OverviewOpenedOrClosed { is_open: true });
        assert!(matches!(
            rest,
            Some(Event::OverviewOpenedOrClosed { is_open: true })
        ));
        assert_eq!(state.state, ViewOutputState::NotViewing);
    }

    #[test]
    fn outputs_state_replicates_an_empty_map_initially() {
        let state = OutputsState::default();
        let events = state.replicate();
        assert!(matches!(
            events.as_slice(),
            [Event::OutputsChanged { outputs }] if outputs.is_empty()
        ));
    }

    #[test]
    fn outputs_state_applies_its_event_and_replicates_it() {
        let mut state = OutputsState::default();
        let outputs = HashMap::from([
            ("DP-2".to_string(), output("DP-2")),
            ("steam".to_string(), output("steam")),
        ]);
        let rest = state.apply(Event::OutputsChanged { outputs });
        assert!(rest.is_none());

        let events = state.replicate();
        let [Event::OutputsChanged { outputs }] = events.as_slice() else {
            panic!("unexpected replicated events: {events:?}");
        };
        let mut names: Vec<_> = outputs.keys().cloned().collect();
        names.sort();
        assert_eq!(names, ["DP-2", "steam"]);
    }

    #[test]
    fn outputs_state_passes_other_events_on() {
        let mut state = OutputsState::default();
        let rest = state.apply(Event::ConfigLoaded { failed: false });
        assert!(matches!(rest, Some(Event::ConfigLoaded { failed: false })));
        assert!(state.outputs.is_empty());
    }

    #[test]
    fn full_state_replicates_both_new_parts() {
        let mut state = EventStreamState::default();
        state.apply(Event::ViewOutputChanged { state: viewing() });
        state.apply(Event::OutputsChanged {
            outputs: HashMap::from([("steam".to_string(), output("steam"))]),
        });

        let events = state.replicate();
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::ViewOutputChanged { state } if *state == viewing())));
        assert!(events.iter().any(
            |e| matches!(e, Event::OutputsChanged { outputs } if outputs.contains_key("steam"))
        ));
    }
}

//! A minimal input backend for driving `State::process_input_event` from tests, so input
//! goes through the real handlers and pointer grabs instead of calling layout directly.

use std::sync::atomic::{AtomicU64, Ordering};

use smithay::backend::input::{
    ButtonState, Device, DeviceCapability, Event, InputBackend, InputEvent, InputTime,
    PointerButtonEvent, PointerMotionEvent, UnusedEvent,
};
use smithay::output::Output;
use smithay::utils::{Logical, Point};

use super::fixture::Fixture;
use crate::input::backend_ext::NiriInputDevice;
use crate::niri::State;

pub const BTN_LEFT: u32 = 0x110;
pub const BTN_RIGHT: u32 = 0x111;

#[derive(Debug)]
pub struct TestInput;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TestDevice;

impl Device for TestDevice {
    fn id(&self) -> String {
        "test-device".to_owned()
    }

    fn name(&self) -> String {
        "test device".to_owned()
    }

    fn has_capability(&self, capability: DeviceCapability) -> bool {
        matches!(capability, DeviceCapability::Pointer)
    }

    fn usb_id(&self) -> Option<(u32, u32)> {
        None
    }

    fn syspath(&self) -> Option<std::path::PathBuf> {
        None
    }
}

impl NiriInputDevice for TestDevice {
    fn output(&self, _state: &State) -> Option<Output> {
        None
    }
}

/// One event type for every event the tests send; the fields a given event does not use
/// keep their defaults.
#[derive(Debug, Clone, Copy, Default)]
pub struct TestEvent {
    time_usec: u64,
    delta: Point<f64, Logical>,
    button: u32,
    pressed: bool,
}

impl Event<TestInput> for TestEvent {
    fn time(&self) -> InputTime {
        InputTime::from_micros(self.time_usec)
    }

    fn device(&self) -> TestDevice {
        TestDevice
    }
}

impl PointerMotionEvent<TestInput> for TestEvent {
    fn delta_x(&self) -> f64 {
        self.delta.x
    }

    fn delta_y(&self) -> f64 {
        self.delta.y
    }

    fn delta_x_unaccel(&self) -> f64 {
        self.delta.x
    }

    fn delta_y_unaccel(&self) -> f64 {
        self.delta.y
    }
}

impl PointerButtonEvent<TestInput> for TestEvent {
    fn button_code(&self) -> u32 {
        self.button
    }

    fn state(&self) -> ButtonState {
        if self.pressed {
            ButtonState::Pressed
        } else {
            ButtonState::Released
        }
    }
}

impl InputBackend for TestInput {
    type Device = TestDevice;
    type KeyboardKeyEvent = UnusedEvent;
    type PointerAxisEvent = UnusedEvent;
    type PointerButtonEvent = TestEvent;
    type PointerMotionEvent = TestEvent;
    type PointerMotionAbsoluteEvent = UnusedEvent;
    type GestureSwipeBeginEvent = UnusedEvent;
    type GestureSwipeUpdateEvent = UnusedEvent;
    type GestureSwipeEndEvent = UnusedEvent;
    type GesturePinchBeginEvent = UnusedEvent;
    type GesturePinchUpdateEvent = UnusedEvent;
    type GesturePinchEndEvent = UnusedEvent;
    type GestureHoldBeginEvent = UnusedEvent;
    type GestureHoldEndEvent = UnusedEvent;
    type TouchDownEvent = UnusedEvent;
    type TouchUpEvent = UnusedEvent;
    type TouchMotionEvent = UnusedEvent;
    type TouchCancelEvent = UnusedEvent;
    type TouchFrameEvent = UnusedEvent;
    type TabletToolAxisEvent = UnusedEvent;
    type TabletToolProximityEvent = UnusedEvent;
    type TabletToolTipEvent = UnusedEvent;
    type TabletToolButtonEvent = UnusedEvent;
    type SwitchToggleEvent = UnusedEvent;
    type SpecialEvent = ();
}

/// Strictly increasing event timestamps, 10 ms apart.
fn next_time_usec() -> u64 {
    static TIME: AtomicU64 = AtomicU64::new(1_000_000);
    TIME.fetch_add(10_000, Ordering::Relaxed)
}

fn event() -> TestEvent {
    TestEvent {
        time_usec: next_time_usec(),
        ..TestEvent::default()
    }
}

fn send(f: &mut Fixture, event: InputEvent<TestInput>) {
    f.niri_state().process_input_event(event);
}

pub fn pointer_motion(f: &mut Fixture, delta: impl Into<Point<f64, Logical>>) {
    let event = TestEvent {
        delta: delta.into(),
        ..event()
    };
    send(f, InputEvent::PointerMotion { event });
}

pub fn pointer_button(f: &mut Fixture, button: u32, pressed: bool) {
    let event = TestEvent {
        button,
        pressed,
        ..event()
    };
    send(f, InputEvent::PointerButton { event });
}

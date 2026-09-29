//! A minimal input backend for driving `State::process_input_event` from tests, so input
//! goes through the real handlers and pointer grabs instead of calling layout directly.

use std::sync::atomic::{AtomicU64, Ordering};

use smithay::backend::input::{
    AbsolutePositionEvent, ButtonState, Device, DeviceCapability, Event, InputBackend, InputEvent,
    InputTime, KeyState, Keycode, PointerButtonEvent, PointerMotionEvent, TouchDownEvent,
    TouchEvent, TouchFrameEvent, TouchSlot, TouchUpEvent, UnusedEvent,
};
use smithay::input::keyboard::FilterResult;
use smithay::output::Output;
use smithay::utils::{Logical, Point, SERIAL_COUNTER};

use super::fixture::Fixture;
use super::test_input_backend::{TestInputBackend, TestKeyboardKeyEvent};
use crate::input::backend_ext::NiriInputDevice;
use crate::niri::State;

pub const BTN_LEFT: u32 = 0x110;
pub const BTN_RIGHT: u32 = 0x111;
pub const BTN_MIDDLE: u32 = 0x112;

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
        matches!(
            capability,
            DeviceCapability::Pointer | DeviceCapability::Touch
        )
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
    slot: Option<u32>,
    /// Absolute position as a fraction of the touch output's size.
    fraction: (f64, f64),
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

impl AbsolutePositionEvent<TestInput> for TestEvent {
    fn x(&self) -> f64 {
        self.fraction.0
    }

    fn y(&self) -> f64 {
        self.fraction.1
    }

    fn x_transformed(&self, width: i32) -> f64 {
        self.fraction.0 * f64::from(width)
    }

    fn y_transformed(&self, height: i32) -> f64 {
        self.fraction.1 * f64::from(height)
    }
}

impl TouchEvent<TestInput> for TestEvent {
    fn slot(&self) -> TouchSlot {
        TouchSlot::from(self.slot)
    }
}

impl TouchDownEvent<TestInput> for TestEvent {}
impl TouchUpEvent<TestInput> for TestEvent {}
impl TouchFrameEvent<TestInput> for TestEvent {}

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
    type TouchDownEvent = TestEvent;
    type TouchUpEvent = TestEvent;
    type TouchMotionEvent = UnusedEvent;
    type TouchCancelEvent = UnusedEvent;
    type TouchFrameEvent = TestEvent;
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

/// Presses or releases the Super key, the headless backend's default mod key.
pub fn super_key(f: &mut Fixture, pressed: bool) {
    const KEY_LEFTMETA: u32 = 125;
    // xkb keycodes are evdev codes plus 8.
    let keycode = Keycode::new(KEY_LEFTMETA + 8);
    let state = if pressed {
        KeyState::Pressed
    } else {
        KeyState::Released
    };
    let time = InputTime::from_micros(next_time_usec());
    let keyboard = f.niri().seat.get_keyboard().unwrap();
    keyboard.input::<(), _>(
        f.niri_state(),
        keycode,
        state,
        SERIAL_COUNTER.next_serial(),
        time,
        |_, _, _| FilterResult::Forward,
    );
}

/// Presses or releases the key with xkb name `name` (such as `ESC` or `LWIN`) through niri's
/// keyboard handler, so binds and compositor UI see it as they would a real key.
pub fn key(f: &mut Fixture, name: &str, pressed: bool) {
    let state = f.niri_state();
    let keyboard = state.niri.seat.get_keyboard().unwrap();
    let code = keyboard
        .with_xkb_state(state, |xkb| {
            let xkb = xkb.xkb().lock().unwrap();
            // SAFETY: the keymap is only read while the xkb lock is held.
            let keymap = unsafe { xkb.keymap() };
            keymap.key_by_name(name)
        })
        .unwrap_or_else(|| panic!("unknown key {name}"));
    let state = if pressed {
        KeyState::Pressed
    } else {
        KeyState::Released
    };
    let event = TestKeyboardKeyEvent {
        time: InputTime::from_micros(next_time_usec()),
        code,
        state,
        count: 1,
    };
    f.niri_state()
        .process_input_event(InputEvent::<TestInputBackend>::Keyboard { event });
}

pub fn pointer_button(f: &mut Fixture, button: u32, pressed: bool) {
    let event = TestEvent {
        button,
        pressed,
        ..event()
    };
    send(f, InputEvent::PointerButton { event });
}

/// Registers the test device, which gives the seat touch capability.
pub fn add_device(f: &mut Fixture) {
    send(f, InputEvent::DeviceAdded { device: TestDevice });
}

/// A touch tap at `fraction` of the touch output's size.
pub fn touch_tap(f: &mut Fixture, fraction: (f64, f64)) {
    let down = TestEvent {
        slot: Some(0),
        fraction,
        ..event()
    };
    send(f, InputEvent::TouchDown { event: down });
    send(f, InputEvent::TouchFrame { event: event() });
    let up = TestEvent {
        slot: Some(0),
        ..event()
    };
    send(f, InputEvent::TouchUp { event: up });
    send(f, InputEvent::TouchFrame { event: event() });
}

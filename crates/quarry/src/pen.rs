//! M0 input spike (macOS): an NSEvent monitor that records the latest pressure
//! and the timestamp of the latest click, for the latency measurement.
//! Tablet pens report pressure on mouse and tablet-point events; Force Touch
//! trackpads report it as pressure events.

use block2::RcBlock;
use objc2::MainThreadMarker;
use objc2_app_kit::{
    NSApplication, NSEvent, NSEventMask, NSEventModifierFlags, NSEventSubtype, NSEventType,
};
use objc2_foundation::{NSPoint, NSProcessInfo};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU8, AtomicU32, AtomicU64, Ordering::Relaxed};

static PRESSURE: AtomicU32 = AtomicU32::new(0);
static SOURCE: AtomicU8 = AtomicU8::new(0);
/// Uptime (s, f64 bits) of the newest click not yet shown in a frame; 0 = none.
static CLICK_AT: AtomicU64 = AtomicU64::new(0);
const SOURCES: [&str; 4] = ["none", "mouse", "pen", "trackpad"];

/// Install the monitor. Call once, on the main thread, after the app starts.
pub fn install() {
    let mask = NSEventMask::LeftMouseDown
        | NSEventMask::LeftMouseDragged
        | NSEventMask::LeftMouseUp
        | NSEventMask::TabletPoint
        | NSEventMask::Pressure;
    let block = RcBlock::new(|event: NonNull<NSEvent>| -> *mut NSEvent {
        // SAFETY: AppKit hands the monitor a live event for the call's duration.
        let e = unsafe { event.as_ref() };
        let source = match e.r#type() {
            NSEventType::TabletPoint => 2,
            NSEventType::Pressure => 3,
            _ if e.subtype() == NSEventSubtype::TabletPoint => 2,
            _ => 1,
        };
        if e.r#type() == NSEventType::LeftMouseDown {
            CLICK_AT.store(e.timestamp().to_bits(), Relaxed);
        }
        PRESSURE.store(e.pressure().to_bits(), Relaxed);
        SOURCE.store(source, Relaxed);
        event.as_ptr() // pass the event on unchanged
    });
    // SAFETY: the block returns the event it was given, which is valid.
    let monitor = unsafe { NSEvent::addLocalMonitorForEventsMatchingMask_handler(mask, &block) };
    // ponytail: the monitor lives for the whole process, so it is never removed.
    std::mem::forget(monitor);
}

/// The latest reading, for the on-screen readout.
pub fn reading() -> String {
    let pressure = f32::from_bits(PRESSURE.load(Relaxed));
    let source = SOURCES[SOURCE.load(Relaxed) as usize];
    format!("pressure {pressure:.3} ({source})")
}

/// Seconds since boot, on the same clock as `NSEvent::timestamp`.
pub fn uptime() -> f64 {
    NSProcessInfo::processInfo().systemUptime()
}

/// Timestamp of a click that arrived since the last call.
pub fn take_click() -> Option<f64> {
    let bits = CLICK_AT.swap(0, Relaxed);
    (bits != 0).then(|| f64::from_bits(bits))
}

/// Queue a synthetic left click stamped with the current uptime.
pub fn post_click(mtm: MainThreadMarker) {
    let event = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
        NSEventType::LeftMouseDown,
        NSPoint::new(400.0, 400.0),
        NSEventModifierFlags::empty(),
        uptime(),
        0,
        None,
        0,
        1,
        1.0,
    );
    if let Some(event) = event {
        NSApplication::sharedApplication(mtm).postEvent_atStart(&event, false);
    }
}

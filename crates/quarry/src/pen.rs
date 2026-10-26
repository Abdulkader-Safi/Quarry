//! M0 pen spike (macOS): an NSEvent monitor that records the latest pressure.
//! Tablet pens report it on mouse and tablet-point events; Force Touch
//! trackpads report it as pressure events.

use block2::RcBlock;
use objc2_app_kit::{NSEvent, NSEventMask, NSEventSubtype, NSEventType};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU8, AtomicU32, Ordering::Relaxed};

static PRESSURE: AtomicU32 = AtomicU32::new(0);
static SOURCE: AtomicU8 = AtomicU8::new(0);
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

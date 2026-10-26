---
id: "m4-pressure-brush-2026-10-26"
status: "todo"
priority: "high"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-27T04:30:00.000Z"
labels: ["m4", "paint", "input"]
order: 43
---

# M4 · Pen pressure changes brush size

FR-PAINT-03.

**Build**
- Wire the M0 pressure source into the paint tool as a brush-size multiplier in whole pixels.
- A size ramp (min/max px) in settings; light press 1 px, heavy 3 px at max size 3.
- Mouse and trackpad use the fixed size; pressure is ignored when absent.
- Windows/Linux use pressure only if the M0 spike passed there.

**Done when**
- [ ] Light and heavy pen strokes produce 1 px and 3 px lines at max size 3.
- [x] Pressure never produces sub-pixel or zero-size strokes.
- [ ] Without a tablet, painting is unchanged.

## Backend done 2026-10-27

- `quarry-ops::paint::pressure_size(pressure, max)`: whole pixels, 1 at a light touch, max at full, never 0, max when there is no tablet. `paint.stroke_3d` takes per-point `pressures`.
- Tested: pressure 0.15 at size 3 paints 1 texel, 1.0 paints a 3x3 stamp.
- Left: feeding the M0 pen readout into the brush in the app, the size ramp in Settings, and the hand test with a tablet.

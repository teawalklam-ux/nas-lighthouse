# NAS Lighthouse Logo Motion Spec

## Motion brief

- Source: `../../src-tauri/icons/icon.png` (512×512 RGBA raster).
- Usage: splash / brand intro, with a static final state suitable for app chrome.
- Personality: **温暖、引导、可靠**。The rounded tile and vivid violet lean friendly; the geometric lighthouse keeps the movement controlled and dependable.
- Preset: Friendly / Approachable, slowed to a 1500 ms splash rhythm.
- Choreography: the white tile establishes the stage, the lighthouse grows upward from its base, its two windows wake in sequence, then the returning beam arcs into place and settles last.

## Geometry

The source is already a clean flat icon, so the fit uses the lowest-complexity primitives and analytic paths rather than a pixel trace:

| Part | SVG id | Geometry | Motion role |
| --- | --- | --- | --- |
| Rounded tile | `#tile` / `#background` | rounded rectangle | stage / anticipation |
| Tower body | `#tower-body` | one trapezoid path | primary rise |
| Lower window | `#tower-window` | rectangle | secondary wake-up |
| Lantern house | `#lantern-house` | one polygonal path | primary arrival |
| Lamp window | `#lamp-window` | one polygonal path | secondary glow |
| Returning light | `#signal` | one chevron path | accent / follow-through |

No trace-derived stair steps or noisy Bézier handles are present. All edges are intentional straight segments or the rounded tile primitive. The violet is sampled directly from the raster: `#6d5dfc`.

## Timeline

Shared clock: **1500 ms**. The broad 20:50:30 shape is 0–300 ms anticipation, 300–1050 ms action, 1050–1500 ms follow-through.

| Time | Beat | Parts | Principles |
| ---: | --- | --- | --- |
| 0–300 ms | dim stage coils inward | tile, house | anticipation, staging |
| 300–630 ms | lighthouse rises | house, tower | timing, slow in/out, solid drawing |
| 630–900 ms | windows wake | lamp and tower windows | overlapping action, secondary action |
| 600–1020 ms | returning beam follows a shallow arc | signal | arcs, staging, slow in/out |
| 1020–1500 ms | offsets and scale settle to exact geometry | all actors | follow-through, appeal |

No two semantic actors share the same entrance and settle beats. All final keyframes return to the exact static SVG geometry.

## Easing tokens

- Enter: `cubic-bezier(0.25, 0.46, 0.45, 0.94)`.
- Narrative: `cubic-bezier(0.34, 0, 0.14, 1)`.
- Settle: `cubic-bezier(0.22, 1, 0.36, 1)`.
- Strong entrance: `cubic-bezier(0.16, 1, 0.3, 1)`.
- Gentle accent pop: `cubic-bezier(0.34, 1.56, 0.64, 1)`.

All timing functions inside keyframes are literal values, so Chromium cannot silently degrade them to linear motion.

## Atomic studies and controls

The standalone HTML includes hover, pulse, arc, and press studies derived from the same SVG; replay, slow motion, and a live speed slider; principle pills; `?t=<ms>` and `?static=1` capture hooks; and `prefers-reduced-motion` support.

## QA record

Geometry and motion evidence is written under `outputs/`.

- Geometry fit: IoU `0.9995`; source-only `10 px`; render-only `17 px`.
- Smoothness verdict: pass. The mark uses a rounded-rectangle primitive plus five low-complexity polygonal shapes; the 512 px browser render shows no staircase contours, chatter, tiny alternating segments, or unintended tangent changes.
- Overlay verdict: centers, extrema, window negative spaces, roof proportions, tower taper, and signal endpoints align with the raster. The tiny residual is antialiasing-only.
- Motion captures: `0, 300, 600, 750, 900, 1200, 1500 ms`, plus live Chromium easing probes at `700, 800, 900 ms`.
- Easing probe: `#signal` computed transforms change nonlinearly (`centerX 546.499 → 537.396 → 535.613`) while opacity eases (`0.810 → 0.966 → 0.996`), confirming the literal keyframe easing is applied rather than a constant-speed segment.
- Motion strip verdict: anticipation reads, the lighthouse is established before its windows, the light returns after the primary tower action, and no part clips the viewBox or hero stage.
- Final Frame Contract, same pipeline: `?t=1500` versus `?static=1` has **0 differing pixels** and maximum channel difference `0` at 358×358.
- Cross-pipeline advisory diff against the separate 512 px static render: mean absolute difference `3.004`, with `1.152%` of pixels differing by at least 25. Visual inspection confirms identical geometry; residue is from browser background, size, shadow, and resampling differences.

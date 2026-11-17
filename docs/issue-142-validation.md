# Issue 142 display validation

[Issue #142](https://github.com/takahirox/snes-rust/issues/142) adds one compact
native **Display: Pixel / CRT** selector beside Scale in Help / Controls. Pixel
remains the default. Only the exact `crt` preference enables CRT on a later visit;
missing, invalid or inaccessible `snes-rust:display` storage falls back safely.
The display preference is independent of ROM identity, SRAM and volume.

## Rendering and failure behavior

The original Canvas 2D path still copies the unmodified Wasm framebuffer into its
existing `ImageData` and calls `putImageData`. In CRT mode, that canvas is also the
texture source for a separate WebGL canvas over exactly the same content box.
The original canvas continues to own layout, focus and input. The overlay ignores
pointer events, is excluded from the accessibility tree and sits below the
placeholder and disclosures. Returning to Pixel removes the overlay and releases
its GPU resources; one context is reused across repeated choices.

The single fragment pass uses opaque, display-encoded RGB input and output.
Filtering deliberately occurs in that same encoded space; this is a visual
interpretation of a consumer television, not a physical NTSC/composite/RGB signal
or colorimetry reconstruction. It has no curvature, flicker, noise, phosphor mask
or extra preset. Five texture samples retain 78% center, 18% nearby and 4% wider
horizontal color, at distances measured in 256 full dots even for 512-sample
hires. A bounded 2.5% wider-color lift provides restrained glow without clipping.
Vertical samples stay on source-row centers. Scanlines follow the source height,
with at most 7% attenuation at row gaps, fading from full strength at 2.5 output
samples per source row to zero at 1.5. This avoids unresolved fine texture in
small viewports. Edges clamp; output stays opaque. Drawing resolution follows the
actual display box with DPR capped at 2 and device texture/renderbuffer limits.

Each presented CRT frame adds one canvas texture upload and one draw, with texture
storage reallocated only on source geometry changes. No CPU filtering, extra
framebuffer allocations per frame, Rust changes or shader dependency were added.
`preserveDrawingBuffer` retains paused frames and GPU canvas captures. Screenshots
capture the composited display; callers capturing a specific canvas can use the
visible `.crt-display` for CRT or `#video` for the unmodified Pixel surface.

Initialization, compile/link, rendering and context failures reveal the current
Pixel frame and update a small status message and the selector's actual state.
They never pause/reset the core, flush audio or write SRAM. Context restoration
allows an explicit retry without overriding Pixel. CPU submission cost is averaged
over 120 presentations; when available, asynchronous disjoint GPU timer queries
also measure the shader draw, with at most four outstanding queries. Sustained
average renderer or full frame workload cost exceeding the existing NTSC field
budget (about 16.64 ms)
returns to Pixel, including core catch-up/audio work when a small shader cost
pushes an already busy device over budget. There is no synchronous GPU wait or
invented benchmark gate.
Without the timer extension only submission cost can be observed; device-wide GPU
congestion cannot be diagnosed precisely. Pixel remains directly selectable.

## Actual browser comparisons

These PNGs are production Chromium screenshots of real Wasm gameplay, not concept
images. Each pair uses the exact same paused source frame, verified by hashing
framebuffer bytes before and after switching. Pausing is only for reproducible
screenshots; separate checks switch during live gameplay. The two dialogue scenes
retain readable Japanese strokes and highlights with modest horizontal softness
and faint row gaps. The hires fixture deliberately stresses alternating colors;
softness is visible without changing the displayed 256:224 shape.

| Scene | Pixel | CRT |
| --- | --- | --- |
| 潮鳴りの密室 — warehouse office dialogue | [Pixel](issue-142/shionari-pixel.png) | [CRT](issue-142/shionari-crt.png) |
| 夜鳴き鐘 — illustrated train prologue | [Pixel](issue-142/yonaki-pixel.png) | [CRT](issue-142/yonaki-crt.png) |
| Original MIT Mode 5 512×224 fixture | [Pixel](issue-142/hires-pixel.png) | [CRT](issue-142/hires-crt.png) |

Reproduce from the repository after building the production site:

```sh
UPDATE_CRT_EVIDENCE=1 npm --prefix web test -- display.spec.js --reporter=list
```

This regenerates the PNGs and [workload measurements](issue-142/render-workload.json).
Normal test runs attach the same evidence to Playwright results without changing
tracked files. Demo PNGs show the unmodified pinned binaries under the existing
[distribution and asset notice](../demos/NOTICE.md); they grant no additional game
rights. Diagnostic and cleared-game ROMs remain test-only and unpublished.

## Workload measurements

Recorded on 2026-11-17, macOS 27.2.1 / Apple M4 Max, headless Chromium
149.0.7827.55. The headless backend reports ANGLE Vulkan SwiftShader, so these are
software-backed WebGL measurements, not physical GPU timings. This environment
does not expose `EXT_disjoint_timer_query`. No real GPU speedup is claimed.

The test runs the actual Yonaki demo at 1280×900, warming each selection for
250 ms, then recording three seconds each of Pixel and CRT at normal speed and
CDP 2×/4× CPU throttling. Renderer timings cover `putImageData` through return for
Pixel, and that same operation through WebGL submission/error checking for CRT.
They exclude asynchronous GPU completion and the framebuffer byte copy common
to both. The full animation callback includes core execution, audio and rendering;
at this browser's approximately 120 Hz rAF cadence, many callbacks run no core
frame. Sub-0.1 ms results reflect browser timer quantization. Figures are
descriptive, with no benchmark pass/fail threshold.

| CPU throttle | Display | Submission mean / p95 (ms) | Whole callback mean / p95 (ms) | Presented / emulated frames (3 s) |
| --- | --- | ---: | ---: | ---: |
| 1× | Pixel | 0.004 / 0.000 | 1.963 / 4.000 | 181 / 181 |
| 1× | CRT | 0.102 / 0.200 | 2.002 / 4.100 | 181 / 181 |
| 2× | Pixel | 0.013 / 0.100 | 4.032 / 8.200 | 181 / 181 |
| 2× | CRT | 0.269 / 0.400 | 4.243 / 8.500 | 180 / 180 |
| 4× | Pixel | 0.033 / 0.200 | 13.523 / 16.600 | 181 / 181 |
| 4× | CRT | 0.569 / 1.000 | 44.055 / 50.100 | 64 / 172 |

At normal speed and 2× throttling, both selections retain the approximately 60 Hz
emulated frame rate. At 4×, the browser can lose presentation frequency, with CRT
adding work
and core catch-up amplifying the delay. Emulated-frame counts include catch-up
and must not be mistaken for presented-frame counts. This configuration is
evidence of a limit, not a claim of midrange-phone performance. The mode can be
switched back immediately; separate injected sustained renderer and full workload
overloads verify automatic fallback. Physical mobile
hardware, GPU profiling and real-TV photographic comparison were not performed.

## Validation on 2026-11-17

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- `cargo test --workspace --locked`: 519 passed.
- `python3 tools/make-fixture.py .cache/smoke.bin` and `cmp` against
  `tests/roms/smoke.bin`: byte-identical.
- `python3 tools/build-test-tools.py`, followed by two consecutive
  `python3 tools/build-cleared-games.py` builds: passed with pinned Castle/Memory
  ROM hashes. The pinned assembler build emitted an existing C string-size
  warning and duplicate C++ linker-library warning; both builds succeeded.
- `python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65"
  --ld65 "$PWD/.cache/tools/cc65/bin/ld65"`: both CPU suites passed.
- `python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"`:
  passed all eleven grayscale bands and final-row persistence.
- `npm --prefix web ci` and
  `RUSTC="$(rustup which rustc)" npm --prefix web run build`: passed, producing
  the production Wasm and unchanged six-file published allowlist.
- `node --check` for app and new tests: passed. Staged whitespace check: passed.
- `UPDATE_CRT_EVIDENCE=1 npm --prefix web test -- --grep-invert '301 seconds'
  --reporter=list`: 145 passed in 6.2 minutes, with zero failures, flakes or skips.
  This final production run includes all 23 display checks and regenerates the
  committed screenshots/workload JSON. The preceding full run passed 144 checks
  before the additional whole-frame overload regression was added.

The new `display.spec.js` covers genuine shader initialization/live presentation,
keyboard type-ahead and selected state, input/audio/SRAM/focus continuity, exact
Pixel bytes, 256→512→256 transitions, paused resize, non-integer scaling,
fullscreen, reset/replacement, retained GPU output after the hidden-tab pause
that existed at the time, and blocked/corrupt/missing preferences and reload.
[Issue #160](https://github.com/takahirox/snes-rust/issues/160) removed the
visibility-triggered pause.
It injects unavailable WebGL, compile/link/draw/error failures and sustained CPU
rendering and whole-frame workload overload, and uses `WEBGL_lose_context` for loss/restoration/retry.
Ten repeated mode cycles check balanced shader/program/texture/buffer cleanup
and reuse of a single context. GPU pixel readback bounds mean brightness change,
checks opacity/orientation and confirms a real canvas stream captures lit output.
Canvas-stream capture requests a fresh presentation while the core remains paused.

The final display suite contains 23 checks. Retina (DPR 3) portrait 320×740 and landscape 667×320 / 568×320 checks retain all
12 touch buttons, live touch input, the volume slider and identical overlay/source
geometry. The narrowest landscape case includes 44px side and 21px bottom safe
areas. Existing browser tests retain keyboard/gamepad/multitouch, audio queue,
SRAM recovery, both demo flows, cleared-game routes and Pages hash/MIME/CSP checks.
Early focused runs exposed fractional positioning, cleanup on compile failure,
native macOS select type-ahead timing and canvas-capture paint synchronization;
these were corrected before the final regression run.

No core/runtime behavior, ROM binary/hash, legal notice, dependency, build workflow
or site asset allowlist changed. The separate source-built SPC700 suite was not
run locally because this diff changes only browser presentation; its existing CI
gate is retained. Optional user-supplied games remain excluded by the documented
`--grep-invert '301 seconds'` command. No remote CI results are claimed.

## After merge

The Issue's main Pages deployment/live-app verification remains pending until
merge and invocation. Check both display choices, live frames and failures across
desktop/mobile sizes at <https://takahirox.github.io/snes-rust/>, recording the real
workflow run and URL/results. If a defect appears, use a separate linked task with
evidence. This local node does not push, open a PR, merge, comment or schedule
background monitoring.

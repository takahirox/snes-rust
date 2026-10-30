# Issue #122: stable cartridge switcher

## Workflow and implementation

Before, **Try demo** and **Open ROM** were separate entry actions. The demo
button became a second Pause/Resume control; local files required another Play
click. Mobile also hid cartridge metadata and changed video size after loading.

Now, one persistent **Inserted cartridge** value and **Change cartridge** trigger
occupy the same space in every state. The native, nonmodal popover has a short
title and exactly two labeled source buttons. It provides focus entry, Tab,
Escape and outside-click dismissal without pausing the game. Menu keyboard
navigation does not send new buttons to the emulator, and releasing a previously
held key still reaches the game. The common toolbar owns the only Play/Pause;
Reset remains the explicit restart action. Reselecting the inserted demo is a
no-op, whether playing or paused. Long filenames and loading messages truncate
visually without moving controls; the full filename remains accessible in the
DOM and the cartridge value's title.

Both sources reuse `loadCartridge`: pause/save, read, normalize local copier
headers, hash, save again, validate, restore, then start automatically. Concurrent
loads are ignored. The Wasm adapter now prepares a separate candidate machine
and commits it only after its battery save restores successfully. The old
machine, framebuffer, name, identity and recoverable SRAM remain inserted on a
failed restore. Save writes, Play, Reset and further replacements stay blocked
until **Retry restoring save** succeeds; retry then commits and auto-starts the
selected cartridge. Failed reads, hashes, ROM validation and save writes retain
the existing game. No emulation hardware behavior changed.

Audio unlock runs synchronously from the source choice, before the native file
picker or asynchronous ROM reads. A second attempt on file selection also helps
browsers retaining activation there. Rejected or pending sound activation never
blocks video or input; the Sound control offers retry/mute. Visiting the page,
opening/dismissing the cartridge picker, and cancelling file selection do not
start emulation. Hidden pages continue to require intentional Play after loading.

This applies the README principles through grouping, fixed geometry and familiar
labels, with no new framework or instructional paragraph in the cartridge UI.
The sole bundled ROM, its 524,288-byte size and pinned SHA-256
`1132c59cfba4cc7b0c1a7d7957b4b5de84c6038e0e061ca2e6372f8b8b4a01ca`,
[distribution/provenance notice](../demos/NOTICE.md), all-rights-reserved game
license, legal disclosure, local-file privacy, CSP and five-file Pages allowlist
remain unchanged. Diagnostic and cleared-game binaries remain test-only. This
change makes no broader ROM compatibility claim or additional Issue #3 claim.

## Actual implementation screenshots

Captured on 2026-10-30 from the production Wasm/site build in Playwright Chromium.
These are actual emulated game frames and live DOM controls, not concept images.
Mobile views use responsive CSS viewport sizes; separate touch-enabled tests
exercise mobile input and emulated safe areas.

| Viewport (CSS pixels) | Playing | Cartridge picker open, gameplay continuing |
| --- | --- | --- |
| Desktop, 1280 × 900 | [Screenshot](issue-122/desktop-playing.png) | [Screenshot](issue-122/desktop-picker.png) |
| Narrow portrait, 320 × 740 | [Screenshot](issue-122/portrait-playing.png) | [Screenshot](issue-122/portrait-picker.png) |
| Short landscape, 667 × 320 | [Screenshot](issue-122/landscape-playing.png) | [Screenshot](issue-122/landscape-picker.png) |

## Executed validation

Environment: macOS arm64, Homebrew Rust 1.94.0 for native checks, rustup Rust
1.99.0 for Wasm and the isolated SPC assembler, Node 26.12.1, Python 3.13.13,
Playwright 1.61.1 / Chromium 149.0.7827.55. Full command output is retained locally in `.cache/issue-122-*.log`;
Playwright attachments, game replay JSON and Memory music/effect WAV are in
`web/test-results` (ignored build/test artifacts).

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `cargo test --workspace --locked` | 519 passed, including the new transactional Wasm-adapter regression |
| `python3 tools/make-fixture.py .cache/smoke.bin` and `cmp tests/roms/smoke.bin .cache/smoke.bin` | Original diagnostic regenerated identically |
| `RUSTC="$(rustup which rustc)" python3 tools/build-test-tools.py --spc` | All pinned build-only assemblers built |
| `python3 tools/build-cleared-games.py`, twice | Castle and Memory matched their pinned hashes on both builds |
| `python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65" --ld65 "$PWD/.cache/tools/cc65/bin/ld65" --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"` | All three source-built CPU/SPC suites passed |
| `python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"` | All eleven HDMA grayscale bands verified |
| `npm --prefix web ci` | Passed |
| `RUSTC="$(rustup which rustc)" npm --prefix web run build` | Production Wasm and five-file Pages site built |
| `npm --prefix web test` | 93 passed in 5.2 minutes; four optional user-ROM tests skipped, no failures |
| Screenshot/geometry rerun with `--grep 'slot and playback geometry.*(1280\|320×740\|667)' --output=../.cache/issue-122-screenshots` | Four passed; screenshots wait for game pixels and use controller input to reach the title |
| `git diff --check`, including staged new files | Passed |

The browser coverage includes actual demo story progression, local game paths,
keyboard/gamepad and all twelve touch inputs, simultaneous presses, fullscreen,
256/512 framebuffer geometry and audio scheduling. New checks cover both source
choices auto-starting, delayed fetch/file reads, duplicate and concurrent choices,
no audio on arrival, rejected/pending audio and Sound recovery, keyboard focus and
Escape/outside dismissal, and file cancellation without changing the running
machine. The cancellation check uses a real Chromium filechooser event followed
by empty-file/cancel events, because Playwright cannot operate the native OS
Cancel button.

Save checks cover per-ROM isolation, copier-header identity, failed writes at
both replacement boundaries, bad fetch/hash/ROM data, and read/base64/size restore
failures both from an empty slot and while replacing either source. Recovery
asserts both the old recoverable SRAM and destination persisted bytes survive.
Fixed geometry is compared across empty, loading, demo, local, paused and error
states at desktop, narrow/short portraits and short landscapes, including emulated
safe areas. Touch tests retain all twelve 44px targets, hit testing, no overflow
and input after picker dismissal. Successful Pages/gameplay paths assert no
404s, CSP failures or console errors; deliberate fetch/storage/CSP fault probes
are separate expected failures.

Intermediate checks found a toolbar wrap/overflow at 568 × 320 with 44px side
and 21px bottom safe areas. Compact landscape toolbar spacing and a fixed video
height budget resolved it. An earlier pending-hash test also needed to avoid
waiting for the loader to finish before releasing its deliberate async barrier.
Neither failure was waived. The first cleared-game build lacked the local ca65
executable; building the documented pinned tools resolved that prerequisite.

## Optional and post-merge checks

Optional user-supplied Gothicvania/original Castle checks are skipped when their
ROM environment variables are absent; no external game binaries were fetched.
Physical iOS Safari, Android Chrome, assistive technology and subjective usability
comparison were not performed. Chromium mobile emulation is the required
pre-merge evidence and does not establish physical-device behavior.

After merge, verify the actual new `main` GitHub Pages run and published
<https://takahirox.github.io/snes-rust/> with both sources, auto-play and dismissal
at desktop/mobile viewport sizes. Record that run, URL and observed results;
create a separate task Issue on failure when invoked. This is pending because
this node is authorized only to implement and commit, without pushing, opening a
PR, merging or posting comments. There is no background monitoring.

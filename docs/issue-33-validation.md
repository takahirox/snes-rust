# Issue #33: HDMA row-end metadata reload

Implemented for [Issue #33](https://github.com/takahirox/snes-rust/issues/33),
starting at checkpoint `6384b4cf112c0c86896958095bd6637b97f37eb5` with production
baseline `edb25aaa2aff9bfe7022252bcb25fb15fa218d5d`. All new instructions, data
and tests are original MIT work. No emulator implementation was consulted.

## Specification facts and ordering derivation

Retrieved the cited references on 2026-10-23. Ignored copies are under
`.cache/issue-33/`; neither reference is redistributed. Both raw hashes match
the Issue #31 record:

| Reference | Version | SHA-256 |
| --- | --- | --- |
| [Anomie's Register Doc](https://raw.githubusercontent.com/gilligan/snesdev/master/docs/snes_registers.txt) | Revision 1157, 2007-07-12 | `19dfdf3f1c206655b8ede11bcd6dc053e36634f876819e40bb39cfb88a237dfd` |
| [Fullsnes](https://problemkaputt.de/fullsnes.htm) | Extracted from no$sns v1.5 | `3853b65a7331bf12eea431ff494a2948a1c78ef22f7e819db4b139fd5a79f6c6` |

Anomie **43x0–43xA** (raw lines 1190–1348) and **DMA AND HDMA → HDMA**
(2422–2513) specify payload direction, bank-preserving increments, decrement
before testing repeat/count, and row-end reload before resuming CPU work. The
scanline algorithm orders payload steps 1–2, decrement/repeat steps 3–4, and
reload/termination step 5. Step 5b identifies the special case as the
“last active HDMA channel for this scanline”; step 5c preserves programmed HDMAEN.
A freshly loaded row waits for the next scanline. Fullsnes **Start/Enable**
(692–720) orders channels low to high; **Channel 0..7 Registers** (725–859)
confirms metadata always reads and `$80` means one transfer plus 127 holds.
Its **Notes** retain uncertainty around mid-frame setup. Fullsnes does not
independently confirm the terminating zero-header quirk.

The numbered algorithm alone does not explicitly nest its steps around the
channel loop. To avoid treating implementation order as evidence, also checked
[AWJ's hardware explanation, post 194923](https://forums.nesdev.org/viewtopic.php?p=194923#p194923),
2017-05-03 UTC (retrieved HTML includes that post). It explicitly states:
“the transfers for all channels come first, followed by loading any indirect
addresses for the next scanline”. This supports two priority-ordered passes:
all eligible payloads, then each channel's decrement/reload. It contradicts
interleaving one channel's reload ahead of a later channel's payload. That
interleaving in earlier code is corrected, not preserved as an earlier contract.

Derived termination sequences, applied during the second pass:

| Current channel / later channel state | Reads and published results |
| --- | --- |
| Direct zero | Read descriptor at T, set `$43xA=0`, advance T once; terminate internally |
| Indirect zero, later enabled and unterminated channel | Read zero at T, low at T+1, high at T+2; publish pointer and advance T three times |
| Indirect zero, no such later channel | Read zero at T as both descriptor and pointer low, high at T+1; publish pointer and advance T twice |
| Later channel is holding | Still active and counts, despite no payload this scanline |
| Later channel also ends this scanline | Still counts when the lower channel reloads; its own zero has not been processed yet |
| Later channel disabled, initially terminated, or terminated on a previous scanline | Does not count |

This applies step 5b at the current priority position, using enabled and internal
unterminated state rather than pre-reading later descriptors or inspecting their
future outcome. The initialization-zero derivation retained from Issue #31
still counts later enabled channels before their initial headers are examined.

## Implementation and retained approximations

`hdma_events` transfers all eligible payloads, then decrements every serviced
channel once, including held lines. A low-seven-bit zero calls the shared
`hdma_reload`, also used by frame initialization. It publishes the descriptor,
reads the indirect pointer before handling zero termination, and sets the next
transfer flag. No new payload runs after reload in the same event. This removes
the initial-only prepared mask: a fresh `$80` is naturally decremented to `$7F`
on its first subsequent payload, while `$81` decrements to `$80` and reloads
immediately. Source registers `$43x2/3/4/7` stay intact.

All metadata uses existing `hdma_read`/`dma_read_a`: actual reads cost eight
modeled master clocks each, preserve status read-clear effects and exclusions,
and increment the 16-bit table address without bank carry. Payloads retain the
existing transfer modes, direction, access checks and independent 16-bit wrap.
No metadata or payload bytes are cached beyond the prepared registers/state.

The scheduler still requests initialization at master clock 24 of scanline 0
and HBlank work at 1096; service occurs at bus boundaries. Global startup,
alignment, full overhead and electrical open-bus values are not established.
Aggregate clock assertions test emulator accounting and APU/raster advancement,
not hardware alignment. Numeric floating status bits in legacy/new synthetic
vectors use the existing deterministic open-bus model; read-clear and address
progression assertions establish separate side effects. CPU guests use broad
polling windows rather than assuming cycle alignment.

## Original regressions and baseline failures

Eleven scheduler tests in `machine/hdma_reload_tests.rs` use natural initialization
and HBlank events through `idle`/bus progress. They never inject HDMA pending,
active, transfer, prepared or line state. Some vectors make CPU register writes
to current table addresses to route a subsequent reload through status or
excluded I/O; synthetic NMI/IRQ flags supply read-side-effect conditions.

Coverage includes direct/indirect table edits after reload with live payloads;
`$01/$02/$80/$81/$82/$FF` counters through their last line; reload after holds;
a subsequent fresh `$80`; direct and both indirect zero sequences; disabled,
paused, initially terminated, previously terminated, holding and same-event
terminating later channels; table/payload wrap; both directions; protected reads;
and NMI/IRQ acknowledgement. A terminating-pointer status vector routes zero
through excluded `$420F`: last-channel high reads `$4210`, whereas a later active
channel also forces a `$4211` read, distinguishing the sequences by both numeric
addresses and NMI/IRQ acknowledgement. Numeric destination, counter, table, pointer,
source-bank and programmed/internal-mask assertions distinguish these states.
The phase test makes a later payload acknowledge IRQ before an earlier reload,
then separately makes two reloads contend for IRQ in channel priority order.
A different-channel 600-byte DMA crosses reload and termination: six CGRAM bytes
plus untouched next slots detect duplicates/loss, final DMA address is `$1258`
and count zero, raster advances, and APU sample cadence stays within one sample.
Its clocks include two direct or five indirect subsequent metadata reads.

Before production edits, the two metadata scheduler tests both failed: direct
table address was `$0102` instead of `$0103`; indirect was `$0103` instead of
`$0106`. Both original CPU guests also failed on unchanged baseline production:

| Guest | Baseline WRAM bytes, hexadecimal (`$1800..$1815`) |
| --- | --- |
| Direct | `03 10 80 A6 5A 11 12 80 11 12 04 10 80 A6 5A 11 12 00 10 00 77 17` |
| Indirect | `03 10 80 02 12 11 12 80 11 12 04 10 80 02 12 11 12 00 10 00 77 17` |

Final CPU-executed results:

| Guest | Final WRAM bytes, hexadecimal (`$1800..$1815`) |
| --- | --- |
| Direct | `04 10 82 A6 5A 11 12 81 66 23 09 10 00 A6 5A 33 34 00 10 00 77 17` |
| Indirect | `06 10 82 00 13 11 12 81 66 23 08 10 00 00 56 33 34 00 10 00 77 17` |

The guests in `tests/hdma_reload.rs` configure in VBlank, synchronize using
executed `$4212` reads, inspect prepared second-row registers, edit the fetched
metadata and live payload, and record CGRAM/register results in WRAM. An extra
HBlank after termination preserves a CPU-written color. Host code only supplies
the cartridge and observes results/deadlines. [Provenance](../tests/roms/PROVENANCE.md#original-hdma-row-end-reload-cpu-guests-approved)
records original MIT instructions/data, with no firmware or commercial ROM.

## Corrections to older expectations

These update the obsolete deferred-row contract; payload/conflict inputs,
read-clear assertions, legal/prohibited access controls, WRAM sentinels, source
register preservation, device progress and byte checks remain present.

| Existing vectors | Derivation for changed assertions |
| --- | --- |
| `hdma_indirect_pointer_fetches_recheck_boundaries_and_wrap` | After `$81` payload, reload zero plus its following high byte in the same event: table advances two more bytes, pointer becomes zero, clocks 32→48 |
| `hdma_tables_feed_blocked_payloads_and_then_terminate` | Same indirect rule; direct reads only zero, clocks 16→24. The extra event is inert |
| `hdma_accessible_indirect_pointer_bytes_keep_values_and_side_effects` | RDIO `$01` now reloads remainder `$0F` from `$4216`, then pointer `$9000` from `$4217/18`; table `$4219`, clocks 56. Fresh `$0F` remains unconsumed |
| `hdma_loaded_status_payload_acknowledges_then_table_terminates` | Payload acknowledges as before, then termination publishes pointer zero/table `$0105` and clocks 48 immediately |
| Complete-table conflict/address/reverse-WMDATA vectors | Repeated row 2 loads descriptor `$02` immediately; held row 4 loads zero and terminates. Counter sequence becomes `$81,$02,$01,$00,$00`; internal active sequence `1,1,1,0,0`; row 5 is inert. Indirect zero adds two reads and overwrites the old payload pointer |
| ROM-table indirect controls | The high byte after zero lies beyond the test table, in unchanged original smoke code: `$A9` at ROM offset `$104`, or `$22` at `$107`, so terminating pointers are `$A900` or `$2200`; WRAM controls yield zero |
| `hdma_reverse_wmdata_reclassifies_each_indirect_destination_and_wraps` | Second payload still performs its address-dependent B-port effect, then zero reload overwrites the indirect address and advances table to `$0105` in that same event |
| Issue #31 metadata-edit / DMA-overlap vectors | Last payload now exposes zero instead of stale `$80`; direct table ends `$0104`, indirect `$0105` with pointer zero. Initial prefetch, live payload and DMA/raster/APU checks stay intact |

For the mode-1 complete-table vector, direct cumulative work clocks are now
24,48,64,72,72; indirect are 40,80,96,112,112. These count actual payload/table
reads and retained scheduling conventions. The original Issue #31 validation
record is marked historical and links here; its frame-initialization contract
is preserved. No test-tool pin, fixture hash, game input, frame count, gameplay,
audio or freeze-control criterion changes.

## Mandatory validation

Rust workspace checks use Homebrew Rust 1.94.0. Wasm and the separately built
SPC assembler select rustup Rust 1.99.0 with the README's
`RUSTC="$(rustup which rustc)"` convention. Logs and reference copies stay ignored
in `.cache/issue-33/`; generated evidence stays in the existing `.cache/`
directories and `web/test-results/`.

| Command | Actual result |
| --- | --- |
| `cargo fmt --all -- --check` | Pass |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Pass |
| `cargo test --workspace --locked` | Pass: 188 tests, no failed/ignored; doctests pass |
| `cargo test --locked -p snes-core --lib machine::` | Pass: 79 tests, including eleven new scheduler tests |
| `cargo test --locked -p snes-core --test dma_access` | Pass: six existing CPU guests |
| `cargo test --locked -p snes-core --test hdma_init` | Pass: two existing CPU guests |
| `cargo test --locked -p snes-core --test hdma_reload` | Pass: two new CPU guests |
| `python3 tools/make-fixture.py .cache/smoke.bin` and `cmp tests/roms/smoke.bin .cache/smoke.bin` | Pass: identical |
| `RUSTC="$(rustup which rustc)" python3 tools/build-test-tools.py --spc` | Pass: all three pinned assemblers built after verified archive recovery |
| `python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65" --ld65 "$PWD/.cache/tools/cc65/bin/ld65" --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"` | Pass: full CPU `$0649`, basic CPU `$0452`, SPC `$0557`; 600 frames each and expected terminal PCs. Preliminary CPU-only run also passed |
| `python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"` | Pass: eleven grayscale bands; final held shade through line 223 |
| `python3 tools/build-cleared-games.py` (twice) | Pass: both fresh builds reproduce both expected hashes |
| `npm --prefix web ci` | Pass: zero reported vulnerabilities |
| `RUSTC="$(rustup which rustc)" npm --prefix web run build` | Pass |
| `(cd web && npx playwright install chromium)` | Pass |
| `SNES_TEST_PORT=18085 npm --prefix web test -- --grep-invert '301 seconds'` | Pass: all 18 checks, 2.6 minutes |

Both games retain 18,100 frames (301.17 emulated seconds), twenty gameplay
windows and freeze-rejection controls. Castle completes all five maps/credits
at frame 11,097. Memory wins/restarts eighteen times; independent music and
finite effects, gesture-activated Web Audio, queue/drift, SRAM failure recovery
and Pages subpath smoke assertions pass. No game tests or inputs were edited.
The alternate port 18085 was selected before starting the browser suite.

Reproduced hashes:

- Smoke: `73c0bad128a030ced5ef160a4e8e3df46de545c3e1c166a44e9d5968798b82fb`.
- Castle: `197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869`.
- Memory: `6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1`.
- Licensed HDMA fixture: `aef339e5d4a7319db31de577b253d977b45f5c45f09adfaa99a302ca300a957a`.

The initial tool builder rejected the SPC archive with a hash mismatch. A
separate bounded download ended with HTTP/2 stream cancellation after 5,813,280
of the advertised 23,872,736 bytes. HTTP/1.1 byte-range responses supplied the
remaining data; the reassembled full archive exactly matched the existing
`979b0fbe19830ffb49d2f187a5512044b7514914f1d289686c340ffd1d6a6e63`
pin before it entered the tool cache. No pin, tool source, licensing decision or
validation script changed. An unused concurrent resume was stopped after that
verification. The initial download/hash rejection is recorded separately from
the final passing gate result. All mandatory gates passed; no pending or skipped
checks remain.

During development, clippy caught an unparenthesized mode-bit expression in a
new test; it was corrected before the passing gate. Initial integration also
exposed obsolete old metadata assertions (corrected above) and floating status
bits in new synthetic vectors; zero-valued payload controls now isolate driven
status effects. The recorded pre-change failures are separate from final gates.

## Remaining limits and inventory

HDMA remains partial: startup/alignment/full overhead, arbitrary mid-frame
enabling, same-channel DMA contention, electrical behavior and the complete
interaction matrix remain outstanding. PAL, display modes and pixel-fetch timing
are outside this change. Limited passes establish neither full hardware accuracy
nor commercial compatibility. Licensing/provenance decisions remain explicit in
the existing audits. Issue #3 is still a historical tracking handoff, not a claim
that partial hardware inventory work completes its original scope.

All 24 inventory scopes/statuses remain unchanged: `cpu`, `bus`, `scheduler`,
`dma`, `hdma`, `controllers`, `ppu-bg`, `ppu-offset`, `ppu-mode7`, `ppu-objects`,
`ppu-color`, `ppu-ports`, `ppu-display`, `spc`, `ipl`, `dsp-noise`, `dsp-pitch`,
`dsp-envelope`, `dsp-echo`, `dsp-timing` remain partial; `dsp-voices` remains
implemented_unverified; `ppu-hires`, `ppu-raster`, `pal` remain unimplemented.
No optional or required post-merge verification is specified by this Issue.

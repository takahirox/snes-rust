# Save state files

Save states restore a completed emulation field or a just-reset machine. Saving
leaves execution and playback intent unchanged. The payload includes hardware
state, cartridge RAM and the retained packed framebuffer. It omits already
generated PCM, Web Audio queues, host controls, display settings, focus and
wall-clock pacing. The core starts with fresh timing and polling accelerators,
rebuilt from the restored hardware state. The currently latched object line and
the object evaluation cache are retained because they can affect rendering.

## Version 1

Integers use little-endian byte order. The file begins with a 56-byte header.

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 8 | ASCII magic `SNESRUST` |
| 8 | 2 | Format version |
| 10 | 32 | SHA-256 of the cartridge ROM after removing any copier header |
| 42 | 1 | Mapping, 0 for LoROM or 1 for HiROM |
| 43 | 1 | Battery flag, 0 or 1 |
| 44 | 4 | SRAM size in bytes |
| 48 | 4 | Uncompressed payload length |
| 52 | 4 | Compressed payload length |

The header is followed by 32 bytes of SHA-256 over the header and compressed
payload, then one zlib stream. No ROM bytes are embedded. The maximum decoded
payload is 1 MiB and the maximum whole file is 1,049,688 bytes. The decoder checks
the selected cartridge, lengths, integrity, stream boundaries and machine data
before preparing a replacement. The field order is defined in the core's
snapshot readers and writers. Changes to that order, the representation, or
emulation semantics that invalidate existing saves require a new format version.
Use a compatible build and the exact ROM revision to restore an older export.
Other emulator state formats are outside this compatibility policy.

## Browser storage and recovery

Every profile, including Save 1, has a stable ID. Its SRAM record stores the
bytes and the last-changed timestamp. The profile list retains the selected ID.

This development-stage update drops legacy SRAM storage compatibility. Old
Save 1 data and its associated browser states may become inaccessible. Export
important SRAM before updating.

State metadata and JPEG previews are paginated independently of the
compressed payloads in IndexedDB. A state belongs to its cartridge and the
profile where it was captured. Exported state files carry the ROM identity and
can be added to another profile for that same ROM. Imports never replace an
existing entry.

Before changing SRAM during a state load, the player makes visible recovery
profiles for any distinct live or persisted SRAM that would be displaced. It
writes each recovery image before publishing its profile, then writes the
restored SRAM before committing the validated machine. A failed write leaves
the running machine intact and the prior saved bytes or completed recovery
profiles available. Selecting a recovery profile restarts the cartridge with
those recovered bytes. Profiles with attached states can be deleted after
their states are removed. Removing a saved ROM from the cartridge picker
leaves all its profiles and snapshots in place.

Use one tab at a time for a given ROM and save profile. Browser persistence and
exit recovery are best-effort. Unsaved progress can be lost if the tab closes
abruptly or a storage write fails, for example when the browser quota is full.
Browser eviction can remove stored saves.

Saves remain in the current browser and site origin. Export backups before
clearing site data or changing browsers. Raw SRAM backups provide a portable
copy of game-native progress, which can outlast snapshot format changes.

See the [Issue #180 validation record](issue-180-validation.md) for the tested
workloads and performance measurements.

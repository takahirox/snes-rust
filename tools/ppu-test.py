#!/usr/bin/env python3
"""Build the pinned zlib HDMA fixture with no third-party bitmap assets.

The 11 grayscale bands must change every 16 scanlines. This verifies scanline
HDMA sequencing, not sub-instruction bus contention or all PPU timing.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import urllib.request

ROOT = Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--bass', required=True, help='Path to bass-untech assembler')
args = parser.parse_args()
build = ROOT / '.cache/ppu-test'
manifest = json.loads((ROOT / 'tests/roms/hdma-source.json').read_text())
for name, sha in manifest['files'].items():
    path = build / name
    path.parent.mkdir(parents=True, exist_ok=True)
    # Always replace patched cache content with verified original source.
    source = path.with_suffix(path.suffix + '.upstream')
    if not source.exists():
        url = f"https://raw.githubusercontent.com/undisbeliever/snes-test-roms/{manifest['commit']}/{name}"
        with urllib.request.urlopen(url, timeout=30) as response:
            data = response.read(2_000_000)
        if hashlib.sha256(data).hexdigest() != sha:
            raise SystemExit(f'Unexpected source hash: {name}')
        source.write_bytes(data)
    data = source.read_bytes()
    if hashlib.sha256(data).hexdigest() != sha:
        raise SystemExit(f'Unexpected cached source hash: {name}')
    path.write_bytes(data)
# Clearly marked local assembly compatibility patch. Empty defines are not
# detected by `defined` with the pinned bass compiler. These are marker macros;
# assigning 1 changes no emitted machine instructions or hardware assertions.
path = build / 'src/_common/memory.inc'
text = path.read_text().replace('    define __MEMORY__.romBlocks.{name}\n',
                               '    define __MEMORY__.romBlocks.{name} = 1\n')
text = text.replace('    define __MEMORY__.ramBlocks.{id}\n',
                    '    define __MEMORY__.ramBlocks.{id} = 1\n')
path.write_text('// Locally altered: explicit values for two marker defines.\n' + text)
font = build / 'gen/textbuffer/font-1bpp-tiles.tiles'
font.parent.mkdir(parents=True, exist_ok=True)
font.write_bytes(bytes(2048))
rom = build / 'hdma.sfc'
subprocess.run([args.bass, '-strict', '-o', str(rom),
                'src/hardware-tests/hdma-timing-demonstration.asm'],
               cwd=build, check=True, timeout=30)
subprocess.run(['cargo', 'build', '--release', '--locked', '-p', 'snes-core'],
               cwd=ROOT, check=True, timeout=120)
ppm = build / 'frame.ppm'
run = subprocess.run([str(ROOT / 'target/release/snes-run'), str(rom),
                      '--frames', '60', '--timeout', '15', '--ppm', str(ppm)],
                     cwd=ROOT, check=True, timeout=20, text=True, capture_output=True)
raw = ppm.read_bytes().split(b'\n', 3)[3]
# Derived directly from CgdataTable: ToPalette(31 - i * 3), 16 lines per entry.
for y in range(224):
    level = 31 - min(y // 16, 10) * 3
    expected = level * 255 // 31
    for x in [0, 128, 255]:
        offset = (y * 256 + x) * 3
        observed = list(raw[offset:offset + 3])
        if observed != [expected] * 3:
            raise SystemExit(f'HDMA mismatch at ({x},{y}): {observed}, expected {[expected]*3}')
result = {'source': manifest['commit'], 'romSha256': hashlib.sha256(rom.read_bytes()).hexdigest(),
          'verified': '11 grayscale bands, each 16 scanlines; final shade persists to line 223',
          'stdout': run.stdout}
(build / 'results.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps(result, indent=2))

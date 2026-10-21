#!/usr/bin/env python3
"""Fetch hash-pinned MIT test source, replace unaudited fonts, build and verify.

No upstream ROM binaries, fonts, firmware, or emulation engines are fetched.
See tests/roms/PROVENANCE.md for the redistribution and toolchain decisions.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import urllib.request

ROOT = Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--ca65', default='ca65')
parser.add_argument('--ld65', default='ld65')
parser.add_argument('--spcasm', help='Also build and run the SPC700 suite with this assembler')
args = parser.parse_args()
manifest = json.loads((ROOT / 'tests/roms/gilyon-source.json').read_text())
build = ROOT / '.cache/hardware-tests'
build.mkdir(parents=True, exist_ok=True)


def command(argv, cwd):
    subprocess.run([str(v) for v in argv], cwd=cwd, check=True, timeout=120)


for name, expected in manifest['files'].items():
    if name.startswith('spctest/') and not args.spcasm:
        continue
    path = build / name
    path.parent.mkdir(parents=True, exist_ok=True)
    if not path.exists():
        url = f"https://raw.githubusercontent.com/gilyon/snes-tests/{manifest['commit']}/{name}"
        with urllib.request.urlopen(url, timeout=30) as response:
            data = response.read(2_000_000)
        if hashlib.sha256(data).hexdigest() != expected:
            raise SystemExit(f'Hash mismatch for {name}; refusing to use source')
        path.write_bytes(data)
    if hashlib.sha256(path.read_bytes()).hexdigest() != expected:
        raise SystemExit(f'Hash mismatch for cached {name}; refusing to use source')

command(['cargo', 'build', '--release', '--locked', '-p', 'snes-core'], ROOT)
runner = ROOT / 'target/release/snes-run'
results = []


def verify(suite, rom, expected_test, pc):
    argv = [runner, rom, '--frames', '600', '--timeout', '30',
            '--expect', f'{hex(4 if suite == "spctest" else 16)}={expected_test & 255}',
            '--expect', f'{hex(5 if suite == "spctest" else 17)}={expected_test >> 8}',
            '--expect-pc', hex(pc), '--dump', '0x10,16']
    completed = subprocess.run([str(v) for v in argv], cwd=ROOT, text=True,
                               capture_output=True, timeout=40)
    print(completed.stdout, end='')
    if completed.returncode:
        raise SystemExit(completed.stderr)
    results.append({'suite': suite, 'sha256': hashlib.sha256(rom.read_bytes()).hexdigest(),
                    'terminal_pc': f'{pc:06x}', 'last_test': f'{expected_test:04x}',
                    'stdout': completed.stdout})


cpu = build / 'cputest'
# Deliberate test-asset alteration: zero bitmap of identical length, no font import.
(cpu / 'font.bin').write_bytes(bytes(2048))
for mode in ['full', 'basic']:
    command([sys.executable, 'make_cpu_tests.py'] + (['--basic'] if mode == 'basic' else []), cpu)
    command([args.ca65] + (['-D', 'basic'] if mode == 'basic' else []) + ['main.asm', '-o', f'{mode}.o'], cpu)
    rom = cpu / f'cputest-{mode}.sfc'
    command([args.ld65, '-C', 'lorom.cfg', '-o', rom, f'{mode}.o'], cpu)
    txt = (cpu / f'tests-{mode}.txt').read_text()
    last = int(re.findall(r'^Test ([0-9a-f]+):', txt, re.M)[-1], 16)
    verify(f'cputest-{mode}', rom, last, 0x81a2)
if args.spcasm:
    spc = build / 'spctest'
    (spc / 'font.bin').write_bytes(bytes(2048))
    command([sys.executable, 'make_tests.py'], spc)
    for i in range(3):
        command([args.spcasm, '-f', 'plain', f'spc_tests{i}.asm', f'spc_tests{i}.spc'], spc)
    command([args.ca65, 'spctest.asm', '-o', 'spctest.o'], spc)
    rom = spc / 'spctest.sfc'
    command([args.ld65, '-C', 'lorom128.cfg', '-o', rom, 'spctest.o'], spc)
    verify('spctest', rom, 0x557, 0x8060)
report = {'source': manifest['commit'], 'results': results}
(build / 'results.json').write_text(json.dumps(report, indent=2) + '\n')
print(f'Verified {len(results)} suites; evidence: {build / "results.json"}')

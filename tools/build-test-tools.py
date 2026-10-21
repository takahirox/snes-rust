#!/usr/bin/env python3
"""Build hash-pinned test assemblers under .cache/tools; install nothing globally.

Python >=3.12, make, C/C++ compilers; --spc additionally needs Rust >=1.99.
The experimental-feature flag applies only to the auxiliary SPC assembler.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tarfile
import urllib.request

ROOT = Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--spc', action='store_true')
args = parser.parse_args()
manifest = json.loads((ROOT / 'tests/roms/toolchains.json').read_text())
cache = ROOT / '.cache/tools'
cache.mkdir(parents=True, exist_ok=True)
for name in ['cc65', 'bass-untech'] + (['spcasm'] if args.spc else []):
    entry = manifest[name]
    archive = cache / f'{name}.tar.gz'
    if not archive.exists():
        with urllib.request.urlopen(entry['url'], timeout=30) as response:
            data = response.read(50_000_000)
        if hashlib.sha256(data).hexdigest() != entry['sha256']:
            raise SystemExit(f'Hash mismatch for {name}')
        archive.write_bytes(data)
    if hashlib.sha256(archive.read_bytes()).hexdigest() != entry['sha256']:
        raise SystemExit(f'Hash mismatch for cached {name}')
    dest = cache / name
    if not dest.exists():
        dest.mkdir()
        with tarfile.open(archive, 'r:gz') as tar:
            # Strip the single enclosing directory, retaining Python's safe data filter.
            for member in tar.getmembers():
                parts = Path(member.name).parts
                if len(parts) <= 1:
                    continue
                member.name = str(Path(*parts[1:]))
                tar.extract(member, dest, filter='data')
    if name == 'cc65':
        command = ['make', '-j4', 'ca65', 'ld65']
        cwd = dest
    elif name == 'bass-untech':
        command = ['make', '-j4']
        cwd = dest / 'bass'
    else:
        command = ['cargo', 'build', '--locked', '--release', '-p', 'spcasm', '--bin', 'spcasm']
        cwd = dest
    env = dict(os.environ)
    if name == 'spcasm':
        env['RUSTC_BOOTSTRAP'] = '1'
    subprocess.run(command, cwd=cwd, env=env, check=True, timeout=240)
    print(f'{name}: built from {entry["commit"]}', flush=True)

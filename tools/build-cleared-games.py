#!/usr/bin/env python3
"""Build the two audited, modified games with ca65/ld65 and Python only.

No prebuilt ROM, ambiguous font, music, firmware or compiler runtime is fetched.
Original converters and replacement lettering: Copyright 2026 snes-rust
contributors, MIT (repository LICENSE). See tests/roms/CLEARED-GAMES.md.
"""
import argparse
import base64
import hashlib
import json
from pathlib import Path
import re
import runpy
import shutil
import struct
import subprocess
import sys
import types
import urllib.request
import xml.etree.ElementTree as ET
import zlib

ROOT = Path(__file__).resolve().parent.parent
MANIFEST = ROOT / 'tests/roms/cleared-games.json'


def fetch_sources(entry, dest):
    for path, sha in entry['files'].items():
        target = dest / path
        if not target.exists():
            url = f"https://raw.githubusercontent.com/{entry['repository']}/{entry['commit']}/{path}"
            with urllib.request.urlopen(url, timeout=30) as response:
                data = response.read(2_000_000)
            if hashlib.sha256(data).hexdigest() != sha:
                raise ValueError(f'Hash mismatch: {url}')
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
        if hashlib.sha256(target.read_bytes()).hexdigest() != sha:
            raise ValueError(f'Hash mismatch: {target}')


def read_pcx(path):
    data = path.read_bytes()
    assert data[0:4] == bytes([10, 5, 1, 8]) and data[65] == 1
    x0, y0, x1, y1 = struct.unpack_from('<4H', data, 4)
    width, height = x1 - x0 + 1, y1 - y0 + 1
    stride = struct.unpack_from('<H', data, 66)[0]
    assert data[-769] == 12
    palette = [r // 8 | (g // 8) << 5 | (b // 8) << 10
               for r, g, b in zip(data[-768::3], data[-767::3], data[-766::3])]
    decoded = bytearray()
    pos = 128
    while len(decoded) < stride * height:
        count, value = 1, data[pos]
        pos += 1
        if value & 0xc0 == 0xc0:
            count, value = value & 63, data[pos]
            pos += 1
        decoded.extend([value] * count)
    assert len(decoded) == stride * height and pos <= len(data) - 769
    return [[palette[i] for i in decoded[y * stride:y * stride + width]]
            for y in range(height)], palette[0]


def planar(tile):
    return bytes(sum(((tile[y * 8 + x] >> plane) & 1) << (7 - x) for x in range(8))
                 for pair in (0, 2) for y in range(8) for plane in (pair, pair + 1))


def convert(pixels, background, path, screen):
    height, width = len(pixels), len(pixels[0])
    assert width % 8 == height % 8 == 0
    tiles = [tuple(pixels[y + dy][x + dx] for dy in range(8) for dx in range(8))
             for y in range(0, height, 8) for x in range(0, width, 8)]
    groups = []
    sets = sorted({frozenset(t) - {background} for t in tiles}, key=lambda s: (-len(s), sorted(s)))
    if not screen:
        groups = [set().union(*sets)]
    else:
        for colors in sets:
            candidates = [g for g in groups if len(g | colors) <= 15]
            if candidates:
                min(candidates, key=lambda g: len(g | colors)).update(colors)
            else:
                groups.append(set(colors))
    assert len(groups) <= 8 and all(len(g) <= 15 for g in groups), (path, groups)
    palettes = [[background] + sorted(g) for g in groups]
    raw, tilemap = bytearray(), bytearray()
    unique = {}
    if screen:
        raw.extend(bytes(32))
        unique[bytes(32)] = 0
    for tile in tiles:
        palette_id = next(i for i, g in enumerate(groups) if set(tile) - {background} <= g)
        encoded = planar([palettes[palette_id].index(c) for c in tile])
        if screen:
            if encoded not in unique:
                unique[encoded] = len(raw) // 32
                raw.extend(encoded)
            index = unique[encoded]
            assert index < 1024, path
            tilemap.extend(struct.pack('<H', index | palette_id << 10))
        else:
            raw.extend(encoded)
    path.with_suffix('.4bpp').write_bytes(raw)
    palette_bytes = b''.join(struct.pack('<16H', *(p + [0] * (16 - len(p)))) for p in palettes)
    path.with_suffix('.clr').write_bytes(palette_bytes.ljust(256, b'\0'))
    if screen:
        path.with_suffix('.map').write_bytes(tilemap)


# Hand-authored 3x5 glyphs. No system font or external glyph bitmap is used.
GLYPHS = dict(zip('ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789:-@.', [
    '010101111101101', '110101110101110', '011100100100011', '110101101101110',
    '111100110100111', '111100110100100', '011100101101011', '101101111101101',
    '111010010010111', '001001001101010', '101101110101101', '100100100100111',
    '101111111101101', '101111111111101', '010101101101010', '110101110100100',
    '010101101111011', '110101110101101', '011100010001110', '111010010010010',
    '101101101101111', '101101101101010', '101101111111101', '101101010101101',
    '101101010010010', '111001010100111', '111101101101111', '010110010010111',
    '110001010100111', '110001010001110', '101101111001001', '111100110001110',
    '011100111101111', '111001010010010', '111101111101111', '111101111001110',
    '000010000010000', '000000111000000', '111101111100011', '000000000000010',
]))


def text(pixels, message, y, scale=2, color=0x7fff):
    x0 = (len(pixels[0]) - len(message) * 4 * scale) // 2
    for i, char in enumerate(message):
        glyph = GLYPHS.get(char, '0' * 15)
        for bit, value in enumerate(glyph):
            if value == '1':
                for dy in range(scale):
                    for dx in range(scale):
                        pixels[y + bit // 3 * scale + dy][x0 + i * 4 * scale + bit % 3 * scale + dx] = color


def castle_assets(dest):
    # The 2015 reset source omits the accumulator-width declaration after REP.
    # Make the intended 16-bit LDA #0 explicit for the pinned ca65 assembler.
    reset = dest / 'snesdev-common/routines/reset-snes.s'
    reset.write_text(reset.read_text().replace('.I16\n\tLDX\t#__STACK_TOP', '.I16\n.A16\n\tLDX\t#__STACK_TOP', 1))
    # Translate old destination-first, bare-bank notation to ca65 2.19's
    # source-first immediate bank notation for the same intended bank copies.
    for path in (dest / 'src').rglob('*.s'):
        path.write_text(re.sub(r'\bMVN\s+(\$7E),\s*(EntityStateBank)', r'MVN #\2, #\1', path.read_text()))
    for path in (dest / 'resources/entities').glob('*.pcx'):
        convert(*read_pcx(path), path, False)
    tiles = dest / 'resources/metatilesets/castle_platformer.pcx'
    convert(*read_pcx(tiles), tiles, True)
    module = runpy.run_path(str(dest / 'utils/map2metatile1x16.py'))
    module['main'](str(tiles.with_suffix('.map')), str(tiles.with_suffix('.metatile1x16')))
    # Exclude the upstream credits PCX/XCF entirely, including the unidentified
    # GIMP Sans glyphs. Keep the authors' attribution in original lettering.
    pixels = [[0] * 256 for _ in range(512)]
    for y, line in [(70, 'YOU WIN'), (240, 'CASTLE PLATFORMER'), (275, 'THE UNDISBELIEVER'),
                    (310, 'ART: JETREL'), (345, 'ART: QUBODUP'), (380, 'ART: JASON-EM'),
                    (425, 'MODIFIED BUILD'), (460, 'SNES-RUST')]:
        text(pixels, line, y)
    convert(pixels, 0, dest / 'resources/images4bpp/credits', True)

    # Adapt the pinned MIT converter's input API to Python's XML parser. Tiled
    # is an authoring tool; no editor or third-party Python package is needed.
    tmx = types.ModuleType('tmx')
    tmx.Layer = type('Layer', (), {})
    tmx.ObjectGroup = type('ObjectGroup', (), {})
    ns = types.SimpleNamespace

    def properties(node):
        return [ns(name=p.get('name'), value=p.get('value')) for p in node.findall('properties/property')]

    def load(path):
        root = ET.parse(path).getroot()
        result = ns(**{k: int(root.get(k)) for k in ['width', 'height', 'tilewidth', 'tileheight']},
                    renderorder=root.get('renderorder'), properties=properties(root), layers=[], tilesets=[])
        for node in root:
            if node.tag == 'tileset':
                result.tilesets.append(ns(firstgid=int(node.get('firstgid')), name=node.get('name')))
            elif node.tag == 'layer':
                layer = tmx.Layer()
                data = node.find('data')
                assert data.get('encoding') == 'base64' and data.get('compression') == 'zlib'
                raw = zlib.decompress(base64.b64decode(data.text))
                layer.tiles = [ns(gid=v[0], hflip=False, vflip=False, dflip=False) for v in struct.iter_unpack('<I', raw)]
                assert all(t.gid < 0x10000000 for t in layer.tiles)
                result.layers.append(layer)
            elif node.tag == 'objectgroup':
                group = tmx.ObjectGroup()
                group.name = node.get('name')
                group.objects = [ns(name=o.get('name', ''), type=o.get('type', ''), properties=properties(o),
                                    **{k: float(o.get(k, '0')) for k in ['x', 'y', 'width', 'height']})
                                 for o in node.findall('object')]
                result.layers.append(group)
        return result

    tmx.TileMap = ns(load=load)
    sys.modules['tmx'] = tmx
    converter = runpy.run_path(str(dest / 'utils/tmx2metamap1x16.py'))
    for path in (dest / 'resources/metatilemaps').glob('*.tmx'):
        converter['main'](str(path), str(path.with_suffix('.metamap1x16')), str(path.with_suffix('.inc')))


def memory_assets(dest):
    for path in [dest / 'resources/images4bpp/cards.pcx', dest / 'resources/tiles4bpp/door.pcx']:
        convert(*read_pcx(path), path, 'images4bpp' in str(path))
    # Replace upstream sprite lettering with our own font and geometric cursor.
    pixels = [[0] * 128 for _ in range(48)]
    # First sixteen pixels are reserved for the 16x16 cursor, not message text.
    for y in range(16):
        for x in range(16):
            pixels[y][x] = 0x001f if (x < 2 and y < 10) or (y < 2 and x < 10) else 0
    # Sprite messages occupy tiles 2..15 on row 0 and tiles 0..15 on rows 2/4.
    # Center title lettering in the 112-pixel message area at x=16.
    for row in pixels[:16]:
        row[16:] = [0] * 112
    title = [[0] * 112 for _ in range(16)]
    text(title, 'PRESS START', 3, 2, 0x001f)
    for y, row in enumerate(title):
        pixels[y][16:] = row
    text(pixels, 'GAME OVER', 19, 2, 0x001f)
    text(pixels, 'YOU WIN', 35, 2, 0x001f)
    convert(pixels, 0, dest / 'resources/tiles4bpp/sprites', False)
    config = dest / 'config.h'
    config.write_text(config.read_text().replace('.define REGION PAL', '.define REGION NTSC'))
    extension = ROOT / 'tools/memory-audio.py'
    expected = json.loads(MANIFEST.read_text())['memory']['audioExtensionSha256']
    if hashlib.sha256(extension.read_bytes()).hexdigest() != expected:
        raise ValueError('Memory audio extension source hash mismatch')
    runpy.run_path(str(extension))['install'](dest)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--ca65', type=Path, default=ROOT / '.cache/tools/cc65/bin/ca65')
    parser.add_argument('--ld65', type=Path, default=ROOT / '.cache/tools/cc65/bin/ld65')
    args = parser.parse_args()
    manifest = json.loads(MANIFEST.read_text())
    results = {}
    for name, entry in manifest.items():
        dest = ROOT / '.cache/cleared-games' / name
        source = ROOT / '.cache/cleared-games/source' / name
        fetch_sources(entry['source'], source)
        fetch_sources(entry['common'], source / 'snesdev-common')
        if dest.exists():
            shutil.rmtree(dest)
        shutil.copytree(source, dest)
        (dest / 'resources/images4bpp').mkdir(parents=True, exist_ok=True)
        if name == 'castle':
            castle_assets(dest)
        else:
            memory_assets(dest)
        # Match the upstream makefile's root-before-subdirectory link order.
        sources = sorted(dest.glob('src/*.s')) + sorted(dest.glob('src/*/*.s'))
        sources += [dest / 'snesdev-common/routines' / (module + '.s') for module in entry['modules']]
        objects = []
        for index, source in enumerate(sources):
            obj = dest / f'{index}.o'
            subprocess.run([str(args.ca65.resolve()), '-I', '.', '-I', 'snesdev-common',
                            '-g', '-o', str(obj), str(source.relative_to(dest))], cwd=dest, check=True, timeout=30)
            objects.append(str(obj))
        output = dest / (name + '.sfc')
        subprocess.run([str(args.ld65.resolve()), '-C', 'snesdev-common/config/LOROM_1MBit_copyright.cfg',
                        '-Ln', str(dest / 'labels.txt'), '-o', str(output), *objects], cwd=dest, check=True, timeout=30)
        data = bytearray(output.read_bytes())
        data[0x7fdc:0x7fe0] = b'\xff\xff\0\0'
        checksum = sum(data) & 0xffff
        struct.pack_into('<HH', data, 0x7fdc, checksum ^ 0xffff, checksum)
        output.write_bytes(data)
        notice = ('Modified source-built game; see tests/roms/CLEARED-GAMES.md.\n'
                  'Game and runtime: The UnDisbeliever, 2015.\n'
                  + ('Castle art: Jetrel, qubodup, Jason-Em / GrafxKid, CC0 1.0.\n'
                     'https://creativecommons.org/publicdomain/zero/1.0/\n' if name == 'castle' else '')
                  + 'Original lettering/converters and Memory sound extension:\n'
                  'snes-rust contributors, 2026, MIT.\n'
                  'Alterations: regenerated graphics/checksum and replacement lettering;\n'
                  'Castle reset/MVN assembler notation; Memory NTSC region declaration\n'
                  'and original card-reveal, match, win and wrong-pair sound effects.\n\n'
                  + (dest / 'LICENSE').read_text() + '\n' + (ROOT / 'LICENSE').read_text())
        (dest / 'NOTICE.txt').write_text(notice)
        sha = hashlib.sha256(data).hexdigest()
        if sha != entry['romSha256']:
            raise ValueError(f'{name} ROM hash mismatch: {sha}')
        results[name] = {'rom': str(output.relative_to(ROOT)), 'sha256': sha, 'bytes': len(data)}
    (ROOT / '.cache/cleared-games/results.json').write_text(json.dumps(results, indent=2) + '\n')
    print(json.dumps(results, indent=2))


if __name__ == '__main__':
    main()

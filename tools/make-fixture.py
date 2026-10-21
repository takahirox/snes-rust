#!/usr/bin/env python3
"""Build our original MIT hardware smoke ROM. No external assembler or assets.

This is a diagnostic, not a homebrew game or an acceptance substitute.
It exercises joypad auto-read, NMI, DMA, planar graphics and SPC-generated BRR
sound. All machine code and pixel/sample data below are authored for this repo.
"""
from pathlib import Path
import hashlib
import sys

rom = bytearray([0xff] * 32768)
code = bytearray()
labels = {}
fixups = []


def emit(*values):
    code.extend(values)


def label(name):
    labels[name] = 0x8000 + len(code)


def branch(op, name):
    emit(op, 0)
    fixups.append((len(code) - 1, name, 'rel'))


def jump(name):
    emit(0x4c, 0, 0)
    fixups.append((len(code) - 2, name, 'abs'))


def write(address, value):
    emit(0xa9, value, 0x8d, address & 255, address >> 8)


emit(0x78, 0xd8)  # SEI, CLD; stay in emulation mode
write(0x2100, 0x80)
write(0x4200, 0)
# A repeating original checker tile, copied to VRAM through DMA.
write(0x2115, 0x80)
write(0x2116, 0)
write(0x2117, 0)
for addr, value in [(0x4300, 1), (0x4301, 0x18), (0x4302, 0),
                    (0x4303, 0x90), (0x4304, 0), (0x4305, 32),
                    (0x4306, 0), (0x420b, 1)]:
    write(addr, value)
write(0x2105, 1)
write(0x2107, 4)  # tilemap at byte 0x800
write(0x212c, 1)
write(0x2121, 0)
for value in [0, 0, 31, 0]:
    write(0x2122, value)
# Upload original SPC program + sample directory/BRR to $0200 using IPL protocol.
spc = bytearray()


def dsp(reg, value):
    spc.extend([0x8f, reg, 0xf2, 0x8f, value, 0xf3])


dsp(0x6c, 0x20)  # echo writes off, sound enabled
dsp(0x0c, 0x60)
dsp(0x1c, 0x60)
dsp(0x5d, 3)     # sample directory at $0300
for reg, value in [(0, 0x60), (1, 0x60), (2, 0), (3, 0x10),
                   (4, 0), (5, 0), (7, 0x7f), (0x4c, 1)]:
    dsp(reg, value)
spc.extend([0x2f, 0xfe])  # BRA forever (DSP continues)
spc.extend(bytes(256 - len(spc)))
spc.extend([0x10, 3, 0x10, 3])
spc.extend(bytes(12))
# BRR loop: alternating four positive and four negative nibbles.
spc.extend([0x83, 0x77, 0x77, 0x99, 0x99, 0x77, 0x77, 0x99, 0x99])
# Upload in two chunks, each <=256 bytes.
label('ipl_wait')
emit(0xad, 0x40, 0x21, 0xc9, 0xaa)
branch(0xd0, 'ipl_wait')
write(0x2142, 0)
write(0x2143, 2)
write(0x2141, 1)
write(0x2140, 0xcc)
for chunk_index, chunk in enumerate([spc[:256], spc[256:]]):
    if chunk_index:
        write(0x2142, 0)
        write(0x2143, 3)
        write(0x2141, 1)
        write(0x2140, 2)  # count would have been zero
    # Compact loop copies from ROM with X=0..chunk length-1.
    emit(0xa2, 0)
    label(f'upload{chunk_index}')
    source = 0x9100 + chunk_index * 256
    emit(0xbd, source & 255, source >> 8, 0x8d, 0x41, 0x21,
         0x8e, 0x40, 0x21)
    label(f'ack{chunk_index}')
    emit(0xec, 0x40, 0x21)
    branch(0xd0, f'ack{chunk_index}')
    emit(0xe8)
    if len(chunk) < 256:
        emit(0xe0, len(chunk))
    branch(0xd0, f'upload{chunk_index}')
write(0x2142, 0)
write(0x2143, 2)
write(0x2141, 0)
write(0x2140, (len(spc[256:]) + 2) & 255)
write(0x2100, 15)
write(0x4200, 0x81)  # NMI + automatic controller read
label('main')
emit(0xcb)  # WAI
jump('main')
label('nmi')
emit(0x48, 0xda, 0x5a, 0xad, 0x10, 0x42)  # save regs; acknowledge
label('joy_wait')
emit(0xad, 0x12, 0x42, 0x29, 1)
branch(0xd0, 'joy_wait')
# Frame counter and last controller byte are externally observable markers.
emit(0xe6, 0x10, 0xad, 0x19, 0x42, 0x85, 0x11)
write(0x2121, 1)
emit(0xa5, 0x10, 0x29, 31, 0x8d, 0x22, 0x21)
write(0x2122, 0)
emit(0x7a, 0xfa, 0x68, 0x40)
for offset, name, mode in fixups:
    value = labels[name]
    if mode == 'rel':
        value -= 0x8000 + offset + 1
        assert -128 <= value <= 127, name
        code[offset] = value & 255
    else:
        code[offset:offset+2] = value.to_bytes(2, 'little')
assert len(code) < 0x1000
rom[:len(code)] = code
rom[0x1000:0x1020] = bytes([0xaa, 0] * 4 + [0x55, 0] * 4 + [0, 0] * 8)
rom[0x1100:0x1100+len(spc)] = spc
rom[0x7fc0:0x7fd5] = b'SNES RUST SMOKE'.ljust(21)
rom[0x7fd5:0x7fdc] = bytes([0x20, 2, 5, 1, 1, 0, 0])
for offset in [0x7fea, 0x7ffa]:
    rom[offset:offset+2] = labels['nmi'].to_bytes(2, 'little')
rom[0x7ffc:0x7ffe] = (0x8000).to_bytes(2, 'little')
# Correct additive checksum/complement, including their own four bytes.
rom[0x7fdc:0x7fe0] = bytes(4)
checksum = (sum(rom) + 510) & 0xffff
rom[0x7fdc:0x7fe0] = (checksum ^ 0xffff).to_bytes(2, 'little') + checksum.to_bytes(2, 'little')
path = Path(sys.argv[1] if len(sys.argv) > 1 else '.cache/fixtures/smoke.sfc')
path.parent.mkdir(parents=True, exist_ok=True)
path.write_bytes(rom)
print(f'{hashlib.sha256(rom).hexdigest()}  {path}')

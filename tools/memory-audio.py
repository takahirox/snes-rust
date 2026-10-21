"""Original MIT sound extension for the cleared Memory Game build.

Copyright (c) 2026 snes-rust contributors. See repository LICENSE.
No external sound driver, firmware, recording or music is used. This emits an
SPC700 program and an original triangular BRR waveform, not host-side audio.
"""

def install(dest):
    code = bytearray()
    labels, fixups = {}, []

    def emit(*values):
        code.extend(values)

    def label(name):
        labels[name] = len(code)

    def branch(op, name):
        emit(op, 0)
        fixups.append((len(code) - 1, name))

    def dsp(reg, value):
        emit(0x8f, reg, 0xf2, 0x8f, value, 0xf3)  # MOV dp,#imm

    # Start muted. One voice, directory $0300, direct gain, no echo/noise.
    for reg, value in [(0x6c, 0x20), (0x0c, 0x70), (0x1c, 0x70),
                       (0x5d, 3), (0, 0x60), (1, 0x60), (2, 0), (3, 4),
                       (4, 0), (5, 0), (7, 0x70), (0x5c, 1)]:
        dsp(reg, value)
    emit(0x8f, 125, 0xfa, 0x8f, 1, 0xf1)  # timer 0: 128*125/1024000 s
    emit(0x8f, 0, 0x20, 0x8f, 0, 0x21)  # last sequence, remaining ticks
    emit(0x8f, 0x55, 0xf4)        # ready acknowledgement to main CPU
    label('wait_zero')
    emit(0xe4, 0xf4)
    branch(0xd0, 'wait_zero')      # wait for the uploader to clear its command
    label('poll')
    emit(0xe4, 0xf4, 0x64, 0x20)  # MOV A,port0; CMP A,last sequence
    branch(0xf0, 'timer')          # BEQ: no new sound
    emit(0xc4, 0x20)              # remember sequence
    emit(0xe4, 0xf5)              # pitch high from main CPU's event
    emit(0x8f, 3, 0xf2, 0xc4, 0xf3)
    dsp(0x5c, 0)                  # release KOFF
    dsp(0x4c, 1)                  # retrigger waveform
    emit(0xe4, 0xfd)              # clear elapsed timer
    emit(0x8f, 8, 0x21)           # effect lasts 125 ms
    label('timer')
    emit(0xe4, 0xfd)              # read and clear timer counter
    branch(0xf0, 'poll')
    emit(0xe4, 0x21)              # already silent?
    branch(0xf0, 'poll')
    emit(0x8b, 0x21)              # DEC remaining ticks
    branch(0xd0, 'poll')
    dsp(0x5c, 1)                  # key off after duration
    branch(0x2f, 'poll')
    for offset, name in fixups:
        distance = labels[name] - offset - 1
        assert -128 <= distance <= 127
        code[offset] = distance & 255
    assert len(code) <= 256
    code.extend(bytes(256 - len(code)))
    code.extend([0x10, 3, 0x10, 3] + [0] * 12)  # start/loop addresses
    # Sixteen signed four-bit samples, filter 0, range 9, end+loop.
    code.extend([0x93, 0x02, 0x46, 0x76, 0x42, 0x0e, 0xca, 0x9a, 0xce])
    (dest / 'memory-spc.bin').write_bytes(code)
    (dest / 'src/audio.s').write_text('''; Original MIT Memory Game sound effects, snes-rust contributors, 2026.
.setcpu "65816"
.a8
.i16
.export MemoryAudioInit, MemoryAudioPlay
.segment "WRAM7F"
Sequence: .res 1
.code
; A8, I16. Called before enabling NMI, DB=$80.
MemoryAudioInit:
    lda #0
    sta f:Sequence
@boot:
    lda $2140
    cmp #$aa
    bne @boot
    lda #0
    sta $2142
    lda #2
    sta $2143
    lda #1
    sta $2141
    lda #$cc
    sta $2140
@ready:
    cmp $2140
    bne @ready
    ldx #0
@first:
    lda f:SpcProgram,x
    sta $2141
    txa
    sta $2140
@ack1:
    cmp $2140
    bne @ack1
    inx
    cpx #256
    bne @first
    lda #0
    sta $2142
    lda #3
    sta $2143
    lda #1
    sta $2141
    lda #2
    sta $2140
@ready2:
    cmp $2140
    bne @ready2
    ldx #0
@second:
    lda f:SpcProgram+256,x
    sta $2141
    txa
    sta $2140
@ack2:
    cmp $2140
    bne @ack2
    inx
    cpx #25
    bne @second
    lda #0
    sta $2142
    lda #2
    sta $2143
    stz $2141
    lda #26
    sta $2140
    ; Explicit SPC acknowledgement: no arbitrary startup delay.
@wait:
    lda $2140
    cmp #$55
    bne @wait
    stz $2140
    rts
; A8=pitch high, I16; preserve A, X, Y and status. DB may be $7E.
MemoryAudioPlay:
    php
    pha
    sta f:$002141
    lda f:Sequence
    inc
    sta f:Sequence
    sta f:$002140
    pla
    plp
    rts
.rodata
SpcProgram: .incbin "memory-spc.bin"
''')
    main = dest / 'src/main.s'
    main.write_text('.import MemoryAudioInit\n' + main.read_text().replace(
        '\tLDA\t#NMITIMEN_VBLANK_FLAG', '\tJSR MemoryAudioInit\n\n\tLDA\t#NMITIMEN_VBLANK_FLAG', 1))
    game = dest / 'src/gameloop.s'
    source = '.import MemoryAudioPlay\n' + game.read_text()
    # Real game events only: card reveal, matching pair, win, and wrong pair.
    for anchor, pitch in [('ROUTINE EnterState_SelectSecond', 4),
                          ('ROUTINE EnterState_WaitForDoors', 5)]:
        assert source.count(anchor) == 1
        source = source.replace(anchor, anchor + f'\n\tLDA #{pitch}\n\tJSR MemoryAudioPlay', 1)
    for anchor, pitch in [('\t\t; not a match', 2),
                          ('\t\tINC\tnCorrectGuesses', 8),
                          ('\t\t\t; Player Wins', 12)]:
        assert source.count(anchor) == 1
        source = source.replace(anchor, f'\t\tLDA #{pitch}\n\t\tJSR MemoryAudioPlay\n' + anchor, 1)
    game.write_text(source)

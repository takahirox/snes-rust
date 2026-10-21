"""Original MIT sound extension for the cleared Memory Game build.

Copyright (c) 2026 snes-rust contributors. See repository LICENSE.
No external sound driver, firmware, recording or composition is used. This emits an
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

    def jump(name):
        emit(0x5f, 0, 0)          # JMP absolute, linked at $0200
        fixups.append((len(code) - 2, name, 'absolute'))

    # Effects on voice 0, score on voice 1. Directory $0400; no echo/noise.
    for reg, value in [(0x6c, 0x20), (0x0c, 0x70), (0x1c, 0x70),
                       (0x5d, 4), (0, 0x60), (1, 0), (2, 0), (3, 4),
                       (4, 0), (5, 0), (7, 0x70), (0x5c, 1),
                       (0x10, 0), (0x11, 0), (0x14, 0), (0x15, 0), (0x17, 0x70)]:
        dsp(reg, value)
    emit(0x8f, 125, 0xfa, 0x8f, 125, 0xfb, 0x8f, 3, 0xf1)  # timer 0: 128*125/1024000 s
    emit(0x8f, 0, 0x20, 0x8f, 0, 0x21)  # last sequence, remaining ticks
    emit(0x8f, 1, 0x22, 0x8f, 0, 0x23, 0x8f, 0, 0x24)  # score ticks/index/enabled
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
    branch(0xf0, 'music')
    emit(0xe4, 0x21)              # already silent?
    branch(0xf0, 'music')
    emit(0x8b, 0x21)              # DEC remaining ticks
    branch(0xd0, 'music')
    dsp(0x5c, 1)                  # key off after duration
    label('music')
    emit(0xe4, 0xf6, 0x64, 0x24)  # port 2: gameplay music enabled
    branch(0xf0, 'music_tick')
    emit(0xc4, 0x24)              # remember start/stop command
    emit(0x8f, 0, 0x23, 0x8f, 1, 0x22)  # restart score from first note
    dsp(0x10, 0)
    dsp(0x11, 0)                  # stop music immediately on title/win/loss
    emit(0xe4, 0xfe)              # discard elapsed time on transition
    label('music_tick')
    emit(0xe4, 0xfe)              # timer 1 drives score independently of effects
    branch(0xf0, 'again')
    emit(0xe4, 0x24)
    branch(0xf0, 'again')
    emit(0x8b, 0x22)              # DEC note duration
    branch(0xd0, 'again')
    emit(0x8f, 16, 0x22)          # 16 * 15.625 ms = quarter-second beat
    emit(0xf8, 0x23)              # MOV X,score byte index
    emit(0xf5, 0xc0, 3)           # MOV A,$03c0+X: pitch low
    emit(0x8f, 0x12, 0xf2, 0xc4, 0xf3)
    emit(0xf5, 0xc1, 3)           # pitch high (zero marks a rest)
    emit(0x8f, 0x13, 0xf2, 0xc4, 0xf3)
    branch(0xf0, 'rest')
    dsp(0x10, 0x28)
    dsp(0x11, 0x28)
    dsp(0x4c, 2)                  # key on only the music voice
    branch(0x2f, 'advance')
    label('rest')
    dsp(0x10, 0)
    dsp(0x11, 0)
    label('advance')
    emit(0xe4, 0x23, 0xbc, 0xbc, 0x28, 31, 0xc4, 0x23)  # index=(index+2)&31
    label('again')
    jump('poll')
    for fixup in fixups:
        offset, name = fixup[:2]
        if len(fixup) == 3:
            code[offset:offset + 2] = (0x200 + labels[name]).to_bytes(2, 'little')
        else:
            distance = labels[name] - offset - 1
            assert -128 <= distance <= 127, (name, distance)
            code[offset] = distance & 255
    assert len(code) <= 0x1c0
    code.extend(bytes(0x1c0 - len(code)))
    # Original 16-beat tune, two answering phrases in C major. Not a quotation
    # or arrangement. 16-sample waveform: pitch=round(Hz * 4096 / 2000).
    notes = [536, 675, 804, 675, 601, 716, 902, 0,
             675, 804, 1012, 804, 601, 804, 1072, 0]
    for pitch in notes:
        code.extend(pitch.to_bytes(2, 'little'))
    code.extend(bytes(512 - len(code)))
    code.extend([0x10, 4, 0x10, 4] + [0] * 12)  # start/loop addresses
    # Sixteen signed four-bit samples, filter 0, range 9, end+loop.
    code.extend([0x93, 0x02, 0x46, 0x76, 0x42, 0x0e, 0xca, 0x9a, 0xce])
    (dest / 'memory-spc.bin').write_bytes(code)
    (dest / 'src/audio.s').write_text('''; Original MIT Memory Game music and sound effects, snes-rust contributors, 2026.
.setcpu "65816"
.a8
.i16
.export MemoryAudioInit, MemoryAudioPlay, MemoryMusicStart, MemoryMusicStop
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
    cpx #SpcProgramEnd-SpcProgram
    bne @first
    lda #0
    sta $2142
    lda #2
    sta $2143
    stz $2141
    lda #.lobyte(SpcProgramEnd-SpcProgram+1)
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
; Called in A8; preserve registers and status, DB may be $7E.
MemoryMusicStart:
    php
    pha
    lda #1
    bra MemoryMusicCommand
MemoryMusicStop:
    php
    pha
    lda #0
MemoryMusicCommand:
    sta f:$002142
    pla
    plp
    rts
.rodata
SpcProgram: .incbin "memory-spc.bin"
SpcProgramEnd:
''')
    main = dest / 'src/main.s'
    main.write_text('.import MemoryAudioInit\n' + main.read_text().replace(
        '\tLDA\t#NMITIMEN_VBLANK_FLAG', '\tJSR MemoryAudioInit\n\n\tLDA\t#NMITIMEN_VBLANK_FLAG', 1))
    game = dest / 'src/gameloop.s'
    source = '.import MemoryAudioPlay, MemoryMusicStart, MemoryMusicStop\n' + game.read_text()
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
    anchor = 'ROUTINE EnterState_OpenAllDoors'
    assert source.count(anchor) == 1
    source = source.replace(anchor, anchor + '\n\tJSR MemoryMusicStart', 1)
    # The title state is assigned at boot, loss and win. Keep those screens silent
    # after the final event's finite effect; no per-frame CPU music synthesis.
    anchor = 'LDX\t#GameState::PRESS_START'
    assert source.count(anchor) == 3
    source = source.replace(anchor, 'JSR MemoryMusicStop\n\t' + anchor)
    game.write_text(source)

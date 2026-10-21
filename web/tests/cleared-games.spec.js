import { test, expect } from '@playwright/test';
import { readFileSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { replay } from './game-replay.js';
import { castleProgression } from './castle-progression.js';

const manifest = JSON.parse(readFileSync(new URL('../../tests/roms/cleared-games.json', import.meta.url)));
async function load(page, name) {
  const buffer = readFileSync(new URL(`../../.cache/cleared-games/${name}/${name}.sfc`, import.meta.url));
  expect(createHash('sha256').update(buffer).digest('hex')).toBe(manifest[name].romSha256);
  await page.addInitScript(() => {
    const original = WebAssembly.instantiate;
    WebAssembly.instantiate = async (...args) => { const result = await original(...args); window.testCore = result.instance.exports; return result; };
    const start = AudioBufferSourceNode.prototype.start;
    AudioBufferSourceNode.prototype.start = function (...args) {
      const samples = this.buffer?.getChannelData(0);
      const right = this.buffer?.getChannelData(1);
      // Original score is centered; effects use the left voice. These observe
      // actual buffers submitted to Web Audio, without replacing the samples.
      if (right?.some((v) => v !== 0)) window.gameMusicBuffers = (window.gameMusicBuffers ?? 0) + 1;
      if (samples?.filter((v, i) => Math.abs(v - right[i]) > .0001).length > 8) window.gameEffectBuffers = (window.gameEffectBuffers ?? 0) + 1;
      if (samples?.some((v) => v !== 0)) window.gameAudioBuffers = (window.gameAudioBuffers ?? 0) + 1;
      return start.apply(this, args);
    };
  });
  await page.goto('/'); await expect(page.locator('#status')).toContainText('Open a local');
  await page.locator('#rom').setInputFiles({ name: `${name}.sfc`, mimeType: 'application/octet-stream', buffer });
  await expect(page.locator('#status')).toContainText('loaded');
}
async function evidence(testInfo, result) {
  const path = testInfo.outputPath('result.json');
  writeFileSync(path, `${JSON.stringify(result, null, 2)}\n`);
  await testInfo.attach('cleared-game-result', { path, contentType: 'application/json' });
}
test('cleared Castle Platformer: five-minute gameplay replay', async ({ page }, testInfo) => {
  test.setTimeout(180000);
  await load(page, 'castle');
  // Player layout comes from entity.h; state comes from this build's ld65 labels.
  const result = await replay(page, { name: 'Castle Platformer (modified)', playing: 0, sound: false,
    state: 0x1421, x: 0x205, y: 0x208, vy: 0x220, ground: 0x222, groundBytes: 2 });
  expect(result.windows).toHaveLength(20);
  expect(result.transitions['0:2']).toBeGreaterThan(0);
  expect(result.transitions['2:0']).toBeGreaterThan(0);
  await evidence(testInfo, result);
});
test('cleared Castle Platformer: controller replay traverses all maps and finishes credits', async ({ page }, testInfo) => {
  test.setTimeout(180000);
  await load(page, 'castle');
  const route = JSON.parse(readFileSync(new URL('./castle-route.json', import.meta.url)));
  expect(route.romSha256).toBe(manifest.castle.romSha256);
  const result = await castleProgression(page, route);
  expect(result.frames).toBe(route.frames);
  await evidence(testInfo, result);
  await page.locator('canvas').screenshot({ path: testInfo.outputPath('credits.png') });
  await page.evaluate(() => document.querySelector('canvas').getContext('2d')
    .putImageData(new ImageData(window.castleWinFrame, 256, 224), 0, 0));
  await page.locator('canvas').screenshot({ path: testInfo.outputPath('you-win.png') });
});
test('cleared Memory Game: five-minute playthroughs using controller inputs', async ({ page }, testInfo) => {
  test.setTimeout(180000);
  await load(page, 'memory');
  const result = await page.evaluate(async () => {
    const core = window.testCore;
    const word = (a) => core.peek(a) | core.peek(a + 1) << 8;
    const require = (ok, message, frame) => { if (!ok) throw new Error(`Memory Game frame ${frame}: ${message}`); };
    const hash = () => {
      const data = new Uint8Array(core.memory.buffer, core.framebuffer(), 256 * 224 * 4);
      let h = 2166136261; for (const b of data) h = Math.imul(h ^ b, 16777619) >>> 0;
      return h >>> 0;
    };
    // Symbols from the pinned build's labels.txt and GameState in gameloop.s.
    const stateAddress = 0x7e2c1e, cursorAddress = 0x7e2c20, guessesAddress = 0x7e2c28;
    let previousState = 2, previousCursor = 0, titleFrames = 0, wins = 0;
    let movements = 0, selections = 0, active = 0, hashes = new Set();
    let nonzero = 0, energy = 0, sampleCount = 0, musicEnergy = 0, effectEnergy = 0;
    const windows = [], transitions = {};
    for (let frame = 0; frame < 18100; frame++) {
      const state = word(stateAddress), cursor = word(cursorAddress);
      titleFrames = state === 2 ? titleFrames + 1 : 0;
      let buttons = 0;
      if (frame % 2 === 0) {
        if (state === 2 && titleFrames > 30) buttons = 0x1000;
        if (state === 6) buttons = 0x8000; // Skip the five-second memorization display.
        if (state === 10 || state === 12) {
          // Read the game's shuffled grid to choose a matching card, then move
          // and select solely with emulated joypad presses. Never write RAM.
          const opened = (i) => core.peek(0x7e2c00 + i);
          const card = (i) => word(0x102 + i * 2);
          const first = word(0x7e2c22);
          const target = Array.from({ length: 30 }, (_, i) => i)
            .find((i) => !opened(i) && (state === 10 || card(i) === card(first)));
          require(target !== undefined, 'no selectable matching card', frame);
          if (Math.floor(cursor / 6) !== Math.floor(target / 6)) buttons = cursor < target ? 0x400 : 0x800;
          else if (cursor !== target) buttons = cursor < target ? 0x100 : 0x200;
          else buttons = 0x8000;
        }
      }
      require(core.run_frame(buttons), 'core execution failed', frame);
      const audio = new Float32Array(core.memory.buffer, core.audio_buffer(), core.audio_len());
      for (const v of audio) {
        require(Number.isFinite(v) && Math.abs(v) <= 1, 'invalid audio sample', frame);
        if (v !== 0) nonzero++;
        energy += v * v;
      }
      for (let i = 0; i < audio.length; i += 2) {
        musicEnergy += audio[i + 1] ** 2;
        effectEnergy += (audio[i] - audio[i + 1]) ** 2;
      }
      sampleCount += audio.length;
      const nextState = word(stateAddress), nextCursor = word(cursorAddress);
      if (titleFrames >= 15 && nextState === 2) {
        require(audio.every((v) => v === 0), 'music/effect persisted on the title', frame);
      }
      if (nextState !== previousState) {
        const key = `${previousState}:${nextState}`;
        transitions[key] = (transitions[key] ?? 0) + 1;
        if (nextState === 12 || nextState === 14) selections++;
        if (nextState === 2 && core.peek(guessesAddress) === 15) wins++;
      }
      if (buttons & 0xf00 && nextCursor !== previousCursor) movements++;
      if (nextState >= 10) active++;
      previousState = nextState; previousCursor = nextCursor;
      if (frame % 6 === 0) hashes.add(hash());
      if (frame % 900 === 899) {
        require(movements >= 20 && selections >= 2 && active >= 100 && hashes.size >= 10,
          `gameplay stalled: movement=${movements}, selections=${selections}, active=${active}, hashes=${hashes.size}`, frame);
        const audioRms = Math.sqrt(energy / sampleCount);
        require(nonzero > 10000 && audioRms > .001, 'game audio went silent', frame);
        const musicRms = Math.sqrt(musicEnergy / (sampleCount / 2));
        const effectRms = Math.sqrt(effectEnergy / (sampleCount / 2));
        require(musicRms > .001 && effectRms > .001, 'music or effects went silent', frame);
        windows.push({ end: frame, movements, selections, active, hashes: hashes.size, nonzero, audioRms, musicRms, effectRms });
        movements = 0; selections = 0; active = 0; hashes = new Set();
        nonzero = 0; energy = 0; sampleCount = 0; musicEnergy = 0; effectEnergy = 0;
      }
      if (frame % 120 === 0) await new Promise((resolve) => setTimeout(resolve, 0));
    }
    return { frames: 18100, emulatedSeconds: 18100 * 1364 * 262 / 21477272, wins, windows, transitions };
  });
  expect(result.wins).toBeGreaterThan(1);
  expect(result.windows).toHaveLength(20);
  await evidence(testInfo, result);
});

test('cleared Memory Game: looping gameplay music and independent finite effects', async ({ page }, testInfo) => {
  await load(page, 'memory');
  const result = await page.evaluate(() => {
    const core = window.testCore;
    const state = () => core.peek(0x7e2c1e);
    const recording = [];
    let capture = false;
    const require = (ok, message) => { if (!ok) throw new Error(message); };
    const step = (buttons = 0) => {
      require(core.run_frame(buttons), 'core execution failed');
      const audio = new Float32Array(core.memory.buffer, core.audio_buffer(), core.audio_len());
      if (capture) {
        const pcm = new Int16Array(audio.length);
        for (let i = 0; i < audio.length; i++) {
          require(Number.isFinite(audio[i]) && Math.abs(audio[i]) <= 1, 'invalid audio sample');
          pcm[i] = Math.round(audio[i] * 32767);
        }
        recording.push(new Uint8Array(pcm.buffer));
      }
      return audio;
    };
    const until = (target) => {
      for (let i = 0; i < 600 && state() !== target; i++) step();
      require(state() === target, `failed to reach game state ${target}`);
    };
    until(2); step(0x1000); until(6); step(0x8000); until(10);
    capture = true;
    // No card has been revealed: any sustained sound here must be music.
    // Recover pitch from interpolated positive zero crossings, not SPC/RAM markers.
    const pitches = [536, 675, 804, 675, 601, 716, 902, 0, 675, 804, 1012, 804, 601, 804, 1072, 0];
    const frequencies = [...new Set(pitches)].filter(Boolean).map((p) => p * 2000 / 4096);
    const runs = [];
    for (let frame = 0; frame < 600; frame++) {
      const audio = step(), crossings = [];
      let energy = 0, effectEnergy = 0;
      for (let i = 2; i < audio.length; i += 2) {
        const a = audio[i - 1], b = audio[i + 1]; // right/music channel
        energy += b * b;
        effectEnergy += (audio[i] - b) ** 2;
        if (a <= 0 && b > 0) crossings.push(i / 2 - a / (b - a));
      }
      // Sequential stereo register writes may differ for one sample.
      require(effectEnergy < .001, `card effect played without an event: frame=${frame}, effectEnergy=${effectEnergy}, energy=${energy}`);
      let note = 0;
      if (energy > .0001 && crossings.length >= 2) {
        const hz = 32000 * (crossings.length - 1) / (crossings.at(-1) - crossings[0]);
        const closest = frequencies.reduce((a, b) => Math.abs(a - hz) < Math.abs(b - hz) ? a : b);
        if (Math.abs(closest - hz) / closest > .02) continue; // boundary between notes
        note = Math.round(closest * 4096 / 2000);
      } else if (energy > .0001) continue;
      const last = runs.at(-1);
      if (last?.note === note) { last.frames++; last.end = frame; }
      else runs.push({ note, frames: 1, start: frame, end: frame });
      require(state() === 10, 'idle gameplay changed state unexpectedly');
    }
    const notes = runs.filter((r) => r.frames >= 6);
    const offset = pitches.findIndex((p, i) => p === notes[0].note && pitches[(i + 1) % 16] === notes[1].note);
    require(offset >= 0 && notes.length >= 38, `missing repeated music phrases: ${JSON.stringify(notes)}`);
    for (let i = 0; i < notes.length; i++) {
      require(notes[i].note === pitches[(offset + i) % 16], `wrong music note ${i}: ${JSON.stringify(notes[i])}`);
      if (i > 0 && i < notes.length - 1) require(notes[i].frames >= 12 && notes[i].frames <= 16, 'note duration drift');
    }
    const musicSeconds = recording.reduce((n, part) => n + part.length, 0) / 128000;
    // Reveal a card: left-minus-right isolates the independent effect voice.
    let effectFrames = 0, lastEffect = -1, musicFramesAfterEffect = 0;
    for (let frame = 0; frame < 120; frame++) {
      const audio = step(frame === 0 ? 0x8000 : 0);
      let effect = 0, music = 0;
      for (let i = 0; i < audio.length; i += 2) {
        effect += (audio[i] - audio[i + 1]) ** 2;
        music += audio[i + 1] ** 2;
      }
      if (effect > .001) { effectFrames++; lastEffect = frame; }
      if (frame > 30 && music > .0001) musicFramesAfterEffect++;
    }
    require(state() === 12 && effectFrames >= 5 && lastEffect < 15, 'card effect absent or not finite');
    require(musicFramesAfterEffect > 60, 'music stopped when the effect ended');
    capture = false;
    require(core.reset(), 'reset failed'); until(2);
    for (let i = 0; i < 60; i++) require(step().every((v) => v === 0), 'reset title is audible');
    // Keep a listenable artifact alongside the numeric waveform evidence.
    const pcmBytes = new Uint8Array(recording.reduce((n, part) => n + part.length, 0));
    let offsetBytes = 0;
    for (const part of recording) { pcmBytes.set(part, offsetBytes); offsetBytes += part.length; }
    let binary = '';
    for (let i = 0; i < pcmBytes.length; i += 16384) binary += String.fromCharCode(...pcmBytes.subarray(i, i + 16384));
    return { rom: 'Memory Game (modified)', musicFrames: 600, musicSeconds,
      notes, effectFrames, lastEffect, musicFramesAfterEffect, resetTitleSilent: true, pcmBase64: btoa(binary) };
  });
  const pcm = Buffer.from(result.pcmBase64, 'base64');
  delete result.pcmBase64;
  const header = Buffer.alloc(44);
  header.write('RIFF'); header.writeUInt32LE(36 + pcm.length, 4); header.write('WAVEfmt ', 8);
  header.writeUInt32LE(16, 16); header.writeUInt16LE(1, 20); header.writeUInt16LE(2, 22);
  header.writeUInt32LE(32000, 24); header.writeUInt32LE(128000, 28);
  header.writeUInt16LE(4, 32); header.writeUInt16LE(16, 34);
  header.write('data', 36); header.writeUInt32LE(pcm.length, 40);
  const wav = Buffer.concat([header, pcm]);
  const path = testInfo.outputPath('music-and-card-effect.wav');
  writeFileSync(path, wav);
  await testInfo.attach('music-and-card-effect', { path, contentType: 'audio/wav' });
  result.audioArtifact = { file: 'music-and-card-effect.wav', sha256: createHash('sha256').update(wav).digest('hex'),
    sampleRate: 32000, channels: 2, seconds: pcm.length / 128000, cardRevealAtSeconds: result.musicSeconds };
  await evidence(testInfo, result);
});

test('cleared Memory Game plays music and finite effects through gesture-activated Web Audio', async ({ page }, testInfo) => {
  await load(page, 'memory');
  const state = () => page.evaluate(() => testCore.peek(0x7e2c1e));
  const audible = () => page.evaluate(() => window.gameAudioBuffers ?? 0);
  const music = () => page.evaluate(() => window.gameMusicBuffers ?? 0);
  const effects = () => page.evaluate(() => window.gameEffectBuffers ?? 0);
  await page.locator('#play').click();
  await expect.poll(state).toBe(2);
  await page.waitForTimeout(300);
  expect(await audible()).toBe(0);
  await page.keyboard.press('Enter', { delay: 80 });
  await expect.poll(state).toBe(6);
  await page.keyboard.press('KeyZ', { delay: 80 });
  await expect.poll(state).toBe(10);
  await expect.poll(music).toBeGreaterThan(0);
  expect(await effects()).toBe(0);
  const beforeMusic = await music();
  await page.waitForTimeout(4500); // More than one full four-second score loop.
  expect(await music()).toBeGreaterThan(beforeMusic + 100);
  expect(await effects()).toBe(0);
  await page.keyboard.press('KeyZ', { delay: 80 });
  await expect.poll(state).toBe(12);
  await expect.poll(effects).toBeGreaterThan(0);
  await page.waitForTimeout(400);
  const afterEffect = await effects(), musicAtEffectEnd = await music();
  await page.waitForTimeout(350);
  expect(await effects()).toBe(afterEffect);
  expect(await music()).toBeGreaterThan(musicAtEffectEnd);
  await page.locator('#play').click();
  await expect(page.locator('#status')).toHaveText('Paused');
  const pathCount = await audible();
  await page.waitForTimeout(100);
  expect(await audible()).toBe(pathCount);
  await page.locator('#reset').click();
  await page.locator('#play').click();
  await expect.poll(state).toBe(2);
  await page.waitForTimeout(300);
  expect(await audible()).toBe(pathCount);
  await page.locator('#play').click();
  await expect(page.locator('#error')).toBeEmpty();
  await evidence(testInfo, { musicBuffers: await music(), effectBuffers: afterEffect,
    idleMusicSampleSeconds: 4.5, effectEndedWhileMusicContinued: true, pauseAndResetSilent: true });
});

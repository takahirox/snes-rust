import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { test } from 'node:test';
import { installAudioProbe } from './browser-audio.mjs';

const require = createRequire(new URL('../../web/package.json', import.meta.url));
const { chromium } = require('@playwright/test');

test('counts a 40 ms audio restart gap after stopping an active buffer and eight queued buffers', async () => {
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage();
    await page.evaluate(installAudioProbe);
    const result = await page.evaluate(async () => {
      window.bench = { audio: 0, gaps: 0, late: 0, gapSeconds: 0, cancelledAudio: 0 };
      const rate = 32000;
      const audio = new OfflineAudioContext(1, rate / 4, rate);
      const buffer = audio.createBuffer(1, 512, rate);
      buffer.getChannelData(0).fill(1);
      const schedule = time => {
        const source = audio.createBufferSource();
        source.buffer = buffer;
        source.connect(audio.destination);
        source.start(time);
        return source;
      };
      const first = 0.016;
      const sources = Array.from({ length: 9 }, (_, index) => schedule(first + index * buffer.duration));
      const flush = audio.suspend(0.024);
      const afterRestart = audio.suspend(0.100);
      const rendering = audio.startRendering();
      await flush;
      window.bench.start = 0;
      const stoppedAt = audio.currentTime;
      for (const source of sources) source.stop();
      const restartAt = stoppedAt + 0.040;
      const restarted = schedule(restartAt);
      const atRestart = { ...window.bench };
      await audio.resume();
      await afterRestart;
      restarted.stop();
      schedule(audio.currentTime + 0.004);
      await audio.resume();

      const samples = (await rendering).getChannelData(0);
      const gap = samples.slice(Math.round(stoppedAt * rate), Math.round(restartAt * rate));
      return {
        atRestart,
        bench: window.bench,
        activeSample: samples[Math.round(0.020 * rate)],
        gapSamples: gap.length,
        gapPeak: Math.max(...gap),
        restartSample: samples[Math.round(0.070 * rate)],
      };
    });
    assert.equal(result.activeSample, 1);
    assert.equal(result.gapSamples, 1280);
    assert.equal(result.gapPeak, 0);
    assert.equal(result.restartSample, 1);
    assert.equal(result.atRestart.audio, 1);
    assert.equal(result.atRestart.gaps, 1);
    assert.equal(result.atRestart.cancelledAudio, 9);
    assert.equal(result.atRestart.late, 0);
    assert.ok(Math.abs(result.atRestart.gapSeconds - 0.040) < 1e-9);
    assert.equal(result.bench.audio, 2);
    assert.equal(result.bench.cancelledAudio, 9);
    assert.equal(result.bench.gaps, 2);
    assert.equal(result.bench.late, 1);
    assert.ok(Math.abs(result.bench.gapSeconds - 0.064) < 1e-9);
  } finally {
    await browser.close();
  }
});

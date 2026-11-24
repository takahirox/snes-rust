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
      const finish = audio.suspend(0.116);
      const rendering = audio.startRendering();
      await flush;
      window.benchAudio.begin();
      const stoppedAt = audio.currentTime;
      for (const source of sources) source.stop();
      const restartAt = stoppedAt + 0.040;
      const restarted = schedule(restartAt);
      await audio.resume();
      await afterRestart;
      restarted.stop();
      schedule(audio.currentTime + 0.004);
      await audio.resume();
      await finish;
      window.benchAudio.finish();
      await audio.resume();

      const samples = (await rendering).getChannelData(0);
      const gap = samples.slice(Math.round(stoppedAt * rate), Math.round(restartAt * rate));
      return {
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
    assert.equal(result.bench.audio, 2);
    assert.equal(result.bench.cancelledAudio, 9);
    assert.equal(result.bench.gaps, 2);
    assert.equal(result.bench.late, 1);
    assert.ok(Math.abs(result.bench.gapSeconds - 0.064) < 1e-9);
  } finally {
    await browser.close();
  }
});

test('counts only audio gaps inside the measured AudioContext window', async t => {
  const browser = await chromium.launch();
  try {
    const cases = [
      { name: 'gap already scheduled during warm-up', before: [0, 0.048], end: 0.064,
        silentSamples: 768, gaps: 1, late: 0 },
      { name: 'gap entirely after the window', before: [0, 0.016, 0.032], during: 0.088,
        end: 0.048, silentSamples: 0, gaps: 0, late: 0 },
      { name: 'gap extending past the window with a restart already scheduled', before: [0, 0.016],
        during: 0.088, end: 0.048, silentSamples: 512, gaps: 1, late: 0 },
      { name: 'gap extending past the window with no restart yet', before: [0, 0.016],
        end: 0.048, silentSamples: 512, gaps: 1, late: 1 },
    ];
    for (const scenario of cases) {
      await t.test(scenario.name, async () => {
        const page = await browser.newPage();
        try {
          await page.evaluate(installAudioProbe);
          const result = await page.evaluate(async ({ before, during, end }) => {
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
            };
            before.forEach(schedule);
            const start = 0.024;
            const opening = audio.suspend(start);
            const closing = audio.suspend(end);
            const rendering = audio.startRendering();
            await opening;
            window.benchAudio.begin();
            if (during !== undefined) schedule(during);
            await audio.resume();
            await closing;
            window.benchAudio.finish();
            // A later start must leave the finished window unchanged.
            schedule(0.120);
            await audio.resume();
            const samples = (await rendering).getChannelData(0);
            const measured = samples.slice(Math.round(start * rate), Math.round(end * rate));
            return { ...window.bench, silentSamples: measured.filter(value => value === 0).length, rate };
          }, scenario);
          assert.equal(result.silentSamples, scenario.silentSamples);
          assert.equal(result.audio, Number(scenario.during !== undefined));
          assert.equal(result.gaps, scenario.gaps);
          assert.equal(result.late, scenario.late);
          assert.ok(Math.abs(result.gapSeconds - scenario.silentSamples / result.rate) < 1e-9);
        } finally {
          await page.close();
        }
      });
    }
  } finally {
    await browser.close();
  }
});

// Opt-in production playback benchmark. Only this local Playwright page is instrumented.
import { createRequire } from 'node:module';
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const require = createRequire(new URL('../../web/package.json', import.meta.url));
const { chromium } = require('@playwright/test');
const [distArg, workload = 'shionari', trialCount = '3', device = 'desktop'] = process.argv.slice(2);
const dist = resolve(distArg);
const root = fileURLToPath(new URL('../../', import.meta.url));
const workloads = JSON.parse(await readFile(new URL('workloads.json', import.meta.url), 'utf8'));
const { rom, window: [warmup, frames], input, route } = workloads[workload];
const masks = Array(warmup + frames).fill(0);
if (route) {
  let start = 0;
  for (const [length, mask] of JSON.parse(await readFile(join(root, route), 'utf8')).segments) {
    masks.fill(Number(mask), start, start + length);
    start += length;
  }
} else {
  for (const [start, end, mask] of input ?? []) masks.fill(mask, start, end);
}

const server = createServer(async (request, response) => {
  const types = { '/': 'text/html', '/app.js': 'text/javascript', '/style.css': 'text/css',
    '/snes_wasm.wasm': 'application/wasm' };
  const type = types[request.url];
  if (!type || request.method !== 'GET') return response.writeHead(404).end();
  try {
    const path = join(dist, request.url === '/' ? 'index.html' : request.url.slice(1));
    response.writeHead(200, { 'Content-Type': type }).end(await readFile(path));
  } catch {
    response.writeHead(500).end();
  }
});
await new Promise(done => server.listen(0, '127.0.0.1', done));
const args = process.env.SNES_BENCH_METAL ? ['--enable-gpu', '--use-angle=metal'] : [];
const browser = await chromium.launch({ headless: !process.env.SNES_BENCH_HEADED, args });
try {
  for (let trial = 0; trial < Number(trialCount); trial++) {
    for (const mode of trial % 2 ? ['crt', 'pixel'] : ['pixel', 'crt']) {
      const mobile = device === 'mobile';
      const context = await browser.newContext(mobile
        ? { viewport: { width: 390, height: 844 }, deviceScaleFactor: 3, isMobile: true, hasTouch: true }
        : { viewport: { width: 1280, height: 800 }, deviceScaleFactor: 2 });
      try {
        const page = await context.newPage();
        const errors = [];
        page.on('pageerror', error => errors.push(error.message));
        const session = await context.newCDPSession(page);
        if (mobile) await session.send('Emulation.setCPUThrottlingRate', { rate: 4 });
        await session.send('Performance.enable');
        await page.addInitScript(({ masks, warmup, frames, mode }) => {
          localStorage.setItem('snes-rust:display', mode);
          const probe = window.bench = {
            frames: 0, timings: [], core: [], arrivals: [], audio: 0, gaps: 0,
            late: 0, gapSeconds: 0, audioEnd: 0, lastFrame: 0,
          };
          const withinWindow = () => probe.frames > warmup && probe.frames <= warmup + frames;
          const instantiate = WebAssembly.instantiate;
          WebAssembly.instantiate = async (...args) => {
            const result = await instantiate(...args);
            const exports = result.instance.exports;
            return { ...result, instance: { exports: {
              ...exports,
              run_frame(buttons) {
                const start = performance.now();
                const ok = exports.run_frame(masks[probe.frames] ?? buttons);
                if (!ok) return ok;
                ++probe.frames;
                if (probe.frames === warmup) probe.start = start;
                if (withinWindow()) probe.core.push(performance.now() - start);
                return ok;
              },
            } } };
          };
          const raf = window.requestAnimationFrame;
          window.requestAnimationFrame = callback => raf.call(window, time => {
            const before = probe.frames;
            const start = performance.now();
            callback(time);
            if (probe.frames > before && probe.frames > warmup && before < warmup + frames) {
              probe.timings.push(performance.now() - start);
              if (probe.lastFrame) probe.arrivals.push(time - probe.lastFrame);
              probe.lastFrame = time;
            }
            if (probe.frames >= warmup + frames && !probe.end) {
              probe.end = performance.now();
              probe.endFields = probe.frames;
            }
          });
          const start = AudioBufferSourceNode.prototype.start;
          AudioBufferSourceNode.prototype.start = function (time, ...args) {
            if (withinWindow()) {
              ++probe.audio;
              if (probe.audioEnd && time > probe.audioEnd + 0.001) {
                ++probe.gaps;
                probe.gapSeconds += time - probe.audioEnd;
                if (this.context.currentTime > probe.audioEnd) ++probe.late;
              }
            }
            probe.audioEnd = time + this.buffer.duration;
            return start.call(this, time, ...args);
          };
        }, { masks, warmup, frames, mode });
        const launchStart = performance.now();
        await page.goto(`http://127.0.0.1:${server.address().port}/`);
        const romBytes = await readFile(join(root, rom));
        await page.locator('#change-cartridge').click();
        await page.locator('input[type=file]').setInputFiles({
          name: 'benchmark.sfc', mimeType: 'application/octet-stream', buffer: romBytes,
        });
        await page.waitForFunction(() => document.querySelector('#status').textContent === 'Playing');
        const launchMs = performance.now() - launchStart;
        await page.waitForFunction(() => window.bench.start);
        const before = await session.send('Performance.getMetrics');
        await page.waitForFunction(() => window.bench.end, null, { timeout: 120_000 });
        const after = await session.send('Performance.getMetrics');
        const result = await page.evaluate(() => ({
          ...window.bench, mode: document.querySelector('#display').value,
          status: document.querySelector('#display-status').textContent,
          error: document.querySelector('#error').textContent,
          audioStatus: document.querySelector('#audio-status').textContent,
          viewport: [innerWidth, innerHeight, devicePixelRatio],
          renderer: (() => {
            const gl = document.createElement('canvas').getContext('webgl');
            const debug = gl?.getExtension('WEBGL_debug_renderer_info');
            return debug ? gl.getParameter(debug.UNMASKED_RENDERER_WEBGL) : null;
          })(),
        }));
        const beforeMetrics = Object.fromEntries(before.metrics.map(({ name, value }) => [name, value]));
        const afterMetrics = Object.fromEntries(after.metrics.map(({ name, value }) => [name, value]));
        const budget = 1000 * 714732 / 2 / 21477272;
        const percentiles = samples => [0.5, 0.95, 0.99].map(quantile =>
          [...samples].sort((a, b) => a - b)[Math.ceil(quantile * samples.length) - 1]);
        console.log(JSON.stringify({
          ...result, errors, launchMs, workload, device, requestedMode: mode, trial: trial + 1,
          chromium: browser.version(),
          guestFrames: result.endFields - warmup,
          guestFps: (result.endFields - warmup) * 1000 / (result.end - result.start),
          workMs: percentiles(result.timings), coreMs: percentiles(result.core),
          arrivalMs: percentiles(result.arrivals),
          workOverBudget: result.timings.filter(ms => ms > budget).length,
          arrivalOverBudget: result.arrivals.filter(ms => ms > budget * 1.1).length,
          jsHeapBytes: afterMetrics.JSHeapUsedSize,
          mainThreadCpuMs: (afterMetrics.ThreadTime - beforeMetrics.ThreadTime) * 1000,
        }));
      } finally {
        await context.close();
      }
    }
  }
} finally {
  await browser.close();
  server.close();
}

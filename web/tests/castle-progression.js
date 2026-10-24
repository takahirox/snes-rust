// Fixed joypad input only: no RAM writes, saved states, cheats or game patches.
// Addresses are from the pinned Castle build's labels.txt and entity.h.
export async function castleProgression(page, route) {
  return page.evaluate(async ({ segments, checkpoints, credits }) => {
    const core = window.testCore;
    const word = (a) => core.peek(a) | core.peek(a + 1) << 8;
    const hash = () => {
      const bytes = new Uint8Array(core.memory.buffer, core.framebuffer(), 256 * 224 * 4);
      let h = 2166136261;
      for (const b of bytes) h = Math.imul(h ^ b, 16777619) >>> 0;
      return h.toString(16).padStart(8, '0');
    };
    const state = () => ({ map: core.peek(0x1420), state: core.peek(0x1421),
      x: word(0x205), y: word(0x208) });
    let frame = 0, previous, lastMap = 0, finished = false;
    const transitions = [], verified = [], maps = Array.from({ length: 5 }, () =>
      ({ frames: 0, movement: 0, jumps: 0, hashes: new Set() }));
    const require = (ok, message) => {
      if (!ok) throw new Error(`Castle progression frame ${frame}: ${message}; ${JSON.stringify(state())}`);
    };
    const byFrame = new Map(checkpoints.map((c) => [c.frame, c]));
    for (const [duration, mask] of segments) {
      const buttons = Number(mask);
      for (let i = 0; i < duration; i++) {
        frame++;
        require(core.run_frame(buttons), new TextDecoder().decode(new Uint8Array(
          core.memory.buffer, core.error_buffer(), core.error_len())));
        const current = state(), ground = word(0x222), vy = (word(0x220) << 16) >> 16;
        require(current.state !== 2, 'unexpected player death');
        require(current.map >= 0 && current.map < 5, 'invalid map');
        if (current.map !== lastMap) {
          require(current.map === lastMap + 1, 'map progression out of order');
          transitions.push({ frame, from: lastMap, to: current.map });
          lastMap = current.map;
        }
        if (current.state === 3) {
          require(current.map === 4, 'finish before the final map');
          finished = true;
        }
        if (current.state === 0) {
          const sample = maps[current.map]; sample.frames++;
          if (previous?.map === current.map && previous.state === 0) {
            if (buttons & 0x300 && Math.abs(current.x - previous.x) > 0 &&
                Math.abs(current.x - previous.x) <= 4) sample.movement++;
            if (buttons & 0x8000 && previous.ground && !ground && vy < 0 && current.y <= previous.y) sample.jumps++;
          }
          if (frame % 6 === 0) sample.hashes.add(hash());
        }
        previous = { ...current, ground };
        const checkpoint = byFrame.get(frame);
        if (checkpoint) {
          for (const field of ['map', 'state', 'x', 'y']) {
            require(current[field] === checkpoint[field], `${checkpoint.label}: ${field} expected ${checkpoint[field]}`);
          }
          if (checkpoint.hash) require(hash() === checkpoint.hash, `${checkpoint.label}: framebuffer mismatch`);
          if (checkpoint.scroll !== undefined) require(word(0x106) === checkpoint.scroll,
            `${checkpoint.label}: credits scroll expected ${checkpoint.scroll}`);
          for (const [address, expected] of Object.entries(checkpoint.ram ?? {})) {
            require(word(Number(address)) === expected, `${checkpoint.label}: RAM ${address} expected ${expected}`);
          }
          if (checkpoint.label === 'Win screen') window.castleWinFrame = new Uint8ClampedArray(
            core.memory.buffer, core.framebuffer(), 256 * 224 * 4).slice();
          verified.push({ ...checkpoint, hash: hash() });
        }
        if (frame % 120 === 0) await new Promise((resolve) => setTimeout(resolve, 0));
      }
    }
    require(verified.length === checkpoints.length, 'unreached checkpoint');
    require(transitions.length === 4 && finished, 'did not traverse all five maps and finish');
    require(word(0x106) === credits.finalScroll, 'credits did not finish scrolling');
    for (const [map, sample] of maps.entries()) {
      require(sample.frames >= 30 && sample.hashes.size >= 3, `map ${map}: insufficient game-derived video`);
      // Map 1 is the vertical drop between the first and third levels.
      if (map !== 1) require(sample.movement >= 20 && sample.jumps >= 1, `map ${map}: no controller-driven traversal`);
    }
    const image = new ImageData(new Uint8ClampedArray(core.memory.buffer,
      core.framebuffer(), 256 * 224 * 4).slice(), 256, 224);
    document.querySelector('canvas').getContext('2d').putImageData(image, 0, 0);
    return { frames: frame, emulatedSeconds: (frame * 357368 - Math.floor(frame / 2) * 4) / 21477272,
      transitions, checkpoints: verified, credits: { state: core.peek(0x1421), scroll: word(0x106), hash: hash() },
      maps: maps.map((m) => ({ ...m, hashes: m.hashes.size })) };
  }, route);
}

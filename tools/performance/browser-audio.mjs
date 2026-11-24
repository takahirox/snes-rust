// Install before the app runs. The playback driver uses one AudioContext
// with full-buffer starts and immediate stops when it flushes the queue.
export function installAudioProbe() {
  const intervals = [];
  const bySource = new WeakMap();
  let context;
  let windowStart;
  let finished = false;
  const { start, stop } = AudioBufferSourceNode.prototype;

  window.benchAudio = {
    begin() {
      windowStart = context?.currentTime ?? 0;
    },
    finish() {
      finished = true;
      if (!context) return;
      const windowEnd = context.currentTime;
      const probe = window.bench;
      const countGap = (from, to, late) => {
        const overlap = Math.min(to, windowEnd) - Math.max(from, windowStart);
        if (overlap <= 0.001) return;
        ++probe.gaps;
        probe.gapSeconds += overlap;
        if (late) ++probe.late;
      };

      // Resolve coverage at close so a future start canceled by a flush
      // cannot split a gap and silence through the closing boundary counts.
      let coveredThrough = 0;
      intervals.sort((a, b) => a.begins - b.begins);
      for (const { begins, end, scheduledAt } of intervals) {
        if (end <= begins) continue;
        countGap(coveredThrough, begins, scheduledAt > coveredThrough);
        coveredThrough = Math.max(coveredThrough, end);
        if (coveredThrough >= windowEnd) return;
      }
      countGap(coveredThrough, windowEnd, true);
    },
  };

  AudioBufferSourceNode.prototype.start = function (time, ...args) {
    if (finished) return start.call(this, time, ...args);
    const now = this.context.currentTime;
    start.call(this, time, ...args);
    context = this.context;
    const begins = Math.max(time, now);
    const interval = { begins, end: begins + this.buffer.duration, scheduledAt: now };
    intervals.push(interval);
    bySource.set(this, interval);
    if (windowStart !== undefined) ++window.bench.audio;
  };

  AudioBufferSourceNode.prototype.stop = function (...args) {
    if (finished) return stop.apply(this, args);
    const now = this.context.currentTime;
    stop.apply(this, args);
    const interval = bySource.get(this);
    if (!interval || now >= interval.end) return;
    interval.end = now;
    if (windowStart !== undefined) ++window.bench.cancelledAudio;
  };
}

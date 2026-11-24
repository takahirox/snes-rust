// Install before the app runs. The playback driver uses one AudioContext
// with full-buffer starts and immediate stops when it flushes the queue.
export function installAudioProbe() {
  const pending = new Map();
  let playedThrough = 0;
  const withinWindow = probe => probe.start !== undefined && probe.end === undefined;
  const { start, stop } = AudioBufferSourceNode.prototype;

  AudioBufferSourceNode.prototype.start = function (time, ...args) {
    const now = this.context.currentTime;
    start.call(this, time, ...args);
    const probe = window.bench;
    if (probe.end !== undefined) return;

    let previousEnd = playedThrough;
    for (const [source, interval] of pending) {
      previousEnd = Math.max(previousEnd, interval.end);
      if (interval.end <= now) {
        playedThrough = Math.max(playedThrough, interval.end);
        pending.delete(source);
      }
    }
    const begins = Math.max(time, now);
    if (withinWindow(probe)) {
      ++probe.audio;
      if (previousEnd && begins - previousEnd > 0.001) {
        ++probe.gaps;
        probe.gapSeconds += begins - previousEnd;
        if (now > previousEnd) ++probe.late;
      }
    }
    pending.set(this, { begins, end: begins + this.buffer.duration });
  };

  AudioBufferSourceNode.prototype.stop = function (...args) {
    const now = this.context.currentTime;
    stop.apply(this, args);
    const probe = window.bench;
    if (probe.end !== undefined) return;

    const interval = pending.get(this);
    if (!interval || now >= interval.end) return;
    pending.delete(this);
    // A canceled future source contributes nothing. An active source
    // contributes only the part that played before the flush.
    if (now > interval.begins) playedThrough = Math.max(playedThrough, now);
    if (withinWindow(probe)) ++probe.cancelledAudio;
  };
}

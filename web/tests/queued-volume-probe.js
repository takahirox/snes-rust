class QueuedProbe extends AudioWorkletProcessor {
  constructor() {
    super(); this.after = Infinity;
    this.port.onmessage = event => { this.after = event.data.after; };
  }
  process(inputs) {
    if (currentTime < this.after) return true;
    const [input, output] = inputs;
    if (input.length !== 2 || output.length !== 2) return true;
    let before = 0, after = 0, n = 0;
    for (let c = 0; c < 2; c++) for (let i = 0; i < input[c].length; i++) {
      before += input[c][i] ** 2; after += output[c][i] ** 2; n++;
    }
    const rms = Math.sqrt(before / n);
    if (rms > .00001) {
      this.port.postMessage({ input: rms, output: Math.sqrt(after / n),
        ratio: Math.sqrt(after / before), sampledAt: currentTime, channels: input.length });
      this.after = Infinity;
    }
    return true; // Outputs remain silent; the production destination is unchanged.
  }
}
registerProcessor('queued-volume-probe', QueuedProbe);

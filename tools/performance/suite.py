#!/usr/bin/env python3
"""Run serial, alternating release benchmark trials and retain JSONL evidence."""
import argparse
import ctypes
import hashlib
import json
import os
import statistics
import subprocess
import sys
import tempfile
from pathlib import Path
from time import perf_counter

ROOT = Path(__file__).resolve().parents[2]
CACHE = ROOT / '.cache/performance'


def reference(library, rom, masks, warmup, frames):
    ref = ctypes.CDLL(str(library))
    ref.bench_load.argtypes = [ctypes.c_char_p, ctypes.c_size_t]
    ref.bench_frame.argtypes = [ctypes.c_uint, ctypes.c_uint]
    for name in ('video', 'wram', 'audio_frames', 'video_frames', 'width', 'height'):
        getattr(ref, f'bench_{name}').restype = ctypes.c_uint
    ref.bench_audio_energy.restype = ctypes.c_double
    start = perf_counter()
    if not ref.bench_load(rom, len(rom)):
        raise RuntimeError('Reference rejected NTSC cartridge')
    load_ms = (perf_counter() - start) * 1000
    elapsed = 0
    previous_audio = 0
    results = []
    try:
        for index, mask in enumerate(masks[:warmup + frames]):
            checkpoint = index >= warmup and (
                (index - warmup + 1) % 120 == 0 or index + 1 == warmup + frames)
            start = perf_counter()
            ref.bench_frame(mask, checkpoint)
            elapsed += perf_counter() - start if index >= warmup else 0
            if index + 1 == warmup:
                previous_audio = ref.bench_audio_frames()
            if checkpoint:
                results.append(dict(frame=index + 1, width=ref.bench_width(),
                                    height=ref.bench_height(), video=ref.bench_video(),
                                    wram=ref.bench_wram(), audio_energy=ref.bench_audio_energy()))
        results.append(dict(frames=frames, warmup=warmup, ms=elapsed * 1000, load_ms=load_ms,
                            audio_frames=ref.bench_audio_frames() - previous_audio,
                            video_frames=ref.bench_video_frames()))
        return results
    finally:
        ref.bench_unload()


def workloads():
    for name, workload in json.loads(Path(__file__).with_name('workloads.json').read_text()).items():
        warmup, frames = workload['window']
        masks = [0] * (warmup + frames)
        if 'route' in workload:
            route = json.loads((ROOT / workload['route']).read_text())
            masks = [int(mask, 0) for length, mask in route['segments'] for _ in range(length)]
        else:
            for start, end, mask in workload.get('input', []):
                masks[start:end] = [mask] * (end - start)
        yield name, ROOT / workload['rom'], masks, warmup, frames


def run_child(command):
    with tempfile.TemporaryFile(mode='w+t') as stdout, tempfile.TemporaryFile(mode='w+t') as stderr:
        start = perf_counter()
        with subprocess.Popen(command, stdout=stdout, stderr=stderr) as child:
            _, status, usage = os.wait4(child.pid, 0)
            child.returncode = os.waitstatus_to_exitcode(status)
        wall_ms = 1000 * (perf_counter() - start)
        if child.returncode:
            stderr.seek(0)
            raise RuntimeError(f"{command[0]} failed: {stderr.read().strip()}")
        stdout.seek(0)
        rows = [json.loads(row) for row in stdout if row.startswith('{')]
    return rows, dict(child_wall_ms=wall_ms, child_cpu_ms=1000 * (usage.ru_utime + usage.ru_stime),
                      peak_rss_bytes=usage.ru_maxrss * (1 if sys.platform == 'darwin' else 1024))


def run_trial(mask_path, name, binary, rom, warmup, frames):
    params = [rom, mask_path, str(warmup), str(frames)]
    if name == 'native-ref':
        # Avoid cross-ROM globals by starting a fresh core process.
        return run_child([sys.executable, __file__, '--reference', binary, *params])
    if name.startswith('wasm'):
        kind = 'snes9x' if name == 'wasm-ref' else 'rust'
        return run_child(['node', Path(__file__).with_name('wasm.mjs'), kind, binary, *params])
    return run_child([binary, *params])


def main():
    if sys.argv[1:2] == ['--reference']:
        library, rom, masks, warmup, frames = sys.argv[2:]
        rows = reference(Path(library), Path(rom).read_bytes(),
                         [int(mask) for mask in Path(masks).read_text().splitlines()],
                         int(warmup), int(frames))
        for row in rows:
            print(json.dumps(row))
        return
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('native', 'wasm'):
        parser.add_argument(f'--{name}', action='append', default=[], metavar='NAME=FILE')
        parser.add_argument(f'--{name}-ref', type=Path)
    parser.add_argument('--trials', type=int, default=3)
    parser.add_argument('--workload', action='append')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    backends = {}
    for kind in ('native', 'wasm'):
        for variant in getattr(args, kind):
            label, sep, binary = variant.partition('=')
            name = f'{kind}-{label}'
            if not sep or not label or not binary or label == 'ref' or name in backends:
                parser.error(f'Expected a unique --{kind} NAME=FILE')
            backends[name] = Path(binary).resolve()
        reference_path = getattr(args, f'{kind}_ref')
        if reference_path:
            backends[f'{kind}-ref'] = reference_path.resolve()
    if not backends or args.trials < 1:
        parser.error('Supply at least one backend and a positive trial count')
    CACHE.mkdir(parents=True, exist_ok=True)
    selected = [work for work in workloads() if not args.workload or work[0] in args.workload]
    if args.workload and set(args.workload) != {work[0] for work in selected}:
        parser.error('Unknown workload')
    with args.output.open('w') as raw, tempfile.TemporaryDirectory(dir=CACHE) as folder:
        mask_path = Path(folder) / 'masks.txt'
        for name, rom, masks, warmup, frames in selected:
            mask_path.write_text(''.join(f'{mask}\n' for mask in masks))
            sha256 = hashlib.sha256(rom.read_bytes()).hexdigest()
            start_methods = list(backends)
            throughputs = {key: [] for key in backends}
            evidence = {}
            for trial in range(args.trials):
                methods = start_methods[trial % len(backends):] + start_methods[:trial % len(backends)]
                for key in methods:
                    rows, utilization = run_trial(mask_path, key, backends[key], rom, warmup, frames)
                    summary = rows[-1]
                    if (summary['frames'] != frames or rows[-2]['frame'] != warmup + frames
                            or summary.get('fields', frames) != frames
                            or summary['audio_frames'] == 0
                            or summary.get('video_frames', warmup + frames) < warmup + frames - 1
                            or not all(row['width'] and row['height'] for row in rows[:-1])):
                        raise RuntimeError(f'Incomplete {key} workload {name}')
                    engine = 'snes9x' if key.endswith('-ref') else 'snes-rust'
                    fingerprint = (summary['audio_frames'], rows[:-1])
                    if evidence.setdefault(engine, fingerprint) != fingerprint:
                        raise RuntimeError(f'Deterministic output differs for {key} workload {name}')
                    fps = 1000 * frames / summary['ms']
                    throughputs[key].append(fps)
                    line = dict(workload=name, sha256=sha256, backend=key, trial=trial + 1,
                                fps=fps, **utilization,
                                checkpoints=rows[:-1], **{k: v for k, v in summary.items()
                                                        if k != 'sha256'})
                    print(json.dumps(line, separators=(',', ':')), file=raw, flush=True)
            estimates = ' '.join(f'{key}={statistics.median(values):.1f}'
                                 for key, values in throughputs.items())
            print(f'{name:<12} {estimates}', flush=True)


if __name__ == '__main__':
    main()

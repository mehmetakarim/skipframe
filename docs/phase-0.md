# Phase 0 — the three risks

Nothing is built on top of these until they pass. Each one is a measurement, not an opinion.

Re-run everything with:

```bash
cargo run --release -p skipframe-gcode --bin sf-gcode -- parse fixtures/bench-prusa.gcode --repeat 5
VITE_BENCH=1 VITE_BENCH_FILE=/abs/path/to/fixtures/bench-prusa.gcode npx tauri dev --release
```

The second command opens the app straight into `src/bench/BenchApp.vue`, which runs risks (b)
and (c) and prints every line to the terminal through the `bench_log` command. The same harness
runs in a plain browser tab at `http://localhost:1420/#bench`, where it falls back to a
synthetic IR so it needs no fixture.

## The test file

There was no real 750 000-path file to hand, so the fixture is generated:

```bash
cargo run --release -p skipframe-gcode --bin sf-gcode -- \
  synth fixtures/bench-prusa.gcode --layers 570 --per-layer 1316 --dialect prusa
```

570 layers x 1316 paths = **750 120 extrusion paths, 25.6 MB**, with a real PrusaSlicer banner,
config block, `;LAYER_CHANGE` / `;Z:` / `;TYPE:` / `;WIDTH:` markers and a layer-change travel.
`--dialect cura|bambu|orca`, `--feature-every` and `--retract-every` produce the other shapes.

**This is synthetic.** It exercises every code path the parser has, but it is more regular than a
real slicer's output. The stress variants below exist to close some of that gap, and the numbers
should be re-taken against a real file before the parser is considered finished.

---

## (a) Parse — must be under 2 s

Measured on the release binary, best of 5 runs, Windows 11, Ryzen-class laptop, NVMe.

| File                                                    |     Paths |           Size |       Parse | Encode to IR | Throughput |
| ------------------------------------------------------- | --------: | -------------: | ----------: | -----------: | ---------: |
| PrusaSlicer                                             |   750 120 |        25.6 MB | **59.2 ms** |       6.7 ms |   433 MB/s |
| Cura (width derived from E)                             |   750 120 |        25.6 MB |     58.4 ms |       4.3 ms |   438 MB/s |
| Bambu Studio                                            |   750 120 |        25.6 MB |     57.6 ms |       4.4 ms |   445 MB/s |
| OrcaSlicer                                              |   750 120 |        25.6 MB |     57.4 ms |       4.7 ms |   447 MB/s |
| Stress: feature marker every 20 paths, retract every 40 |   750 120 |        27.8 MB |     66.0 ms |       4.9 ms |   421 MB/s |
| Scale: 2000 layers                                      | 3 000 000 |       107.2 MB |    245.5 ms |      17.3 ms |   437 MB/s |
| `.gcode.3mf`, plate 1 of 2 (includes inflate)           |   750 120 | 17.9 MB zipped |     96.1 ms |            — |          — |

SHA-256 for the cache key costs 13.5 ms on the 25.6 MB file and 54 ms on the 107 MB one.

**Verdict: pass, by roughly 30x.** Throughput is flat from 25 MB to 107 MB, so the streaming
design holds and there is no file-size limit to defend.

Correctness checks that came free with the measurement: 570 layers found, 750 120 extruding
segments plus exactly one travel per layer, Z range 0.2–114.0 mm, printer resolved from
`printer_model = MK4IS` to the Prusa MK4 profile, bed read from `bed_shape`. In the stress file
the retract / travel / prime triplets produced exactly one travel segment each.

### The IPC round trip

Parsing is only part of what the user waits for. Full round trip from the front end —
hash, parse, encode, write cache, hand 22.5 MB across IPC, wrap in TypedArrays:

| Build                                       |  Round trip |
| ------------------------------------------- | ----------: |
| `tauri dev --release`                       |  **247 ms** |
| `tauri dev` (debug shell, optimised parser) | 843–1070 ms |

The parser is compiled at `opt-level = 3` even in dev builds, so the gap between the two rows is
the unoptimised shell and IPC layer, not parsing. 247 ms is what a user actually waits for on a
cold open of a 750 000-path file, cache miss included.

---

## (b) Render — must hold 60 fps while scrubbing

570 frames, one per layer, sweeping the whole print. Canvas 1080x1920 at pixel ratio 1 — the
export resolution, which is heavier than the on-screen viewport. Travels hidden. Measured inside
the app's WebView2.

| Metric                                       | Value                                         |
| -------------------------------------------- | --------------------------------------------- |
| Frame interval                               | p50 **4.20 ms**, p95 4.30 ms, max 17.0 ms     |
| Frames within a 60 Hz budget                 | 100 %                                         |
| Mean rate                                    | ~238 fps                                      |
| `renderer.render()` CPU cost                 | p50 0.10 ms, p95 0.20 ms                      |
| GPU time (`EXT_disjoint_timer_query_webgl2`) | p50 1.04 ms, p95 2.03 ms                      |
| Draw calls per frame                         | 3 (print, bed grid, bed outline)              |
| Context                                      | WebGL2, ANGLE / D3D11, NVIDIA RTX 5060 Laptop |

**Verdict: pass, by roughly 4x on the GPU and far more on the CPU.**

What makes it cheap: one merged `LineSegments` geometry, animated only by `setDrawRange`. The
position array is the parser's buffer with no copy. Feature colour is one `Uint8` attribute per
vertex (1.5 MB) sampled against a 16x1 palette texture, so recolouring costs a 64-byte upload
instead of rewriting the vertex buffer. Travels are hidden by collapsing them outside the clip
volume in the vertex shader rather than by splitting the geometry into a second draw call.

The measurement was taken on a discrete GPU. Integrated graphics will be slower, and the
headroom above is the argument that it will still clear 60 fps — but that is an inference, not a
measurement.

---

## (c) WebCodecs H.264 — the one that decides the export architecture

180 frames of 1080x1920 at 60 fps, taken straight off the render canvas with
`new VideoFrame(canvas, { timestamp })` — no `readPixels`, no IPC — encoded with `VideoEncoder`
and muxed with `mp4-muxer`.

### Windows (WebView2 / Chromium)

| Probe                                         | Result    |
| --------------------------------------------- | --------- |
| `VideoEncoder` present                        | yes       |
| `avc1.640028` (High 4.2), `prefer-hardware`   | supported |
| `avc1.640028` (High 4.2), `no-preference`     | supported |
| `avc1.4d0028` (Main 4.0), `prefer-hardware`   | supported |
| `avc1.42002a` (Baseline 4.0), `no-preference` | supported |

| Metric      | Value                                   |
| ----------- | --------------------------------------- |
| Encode rate | **262 fps** (render + capture + encode) |
| 180 frames  | 0.69 s                                  |
| Muxing      | 7 ms total                              |
| Output      | 4.77 MB, 2 keyframes                    |

The output was written to disk and read back with `ffprobe`, so "it produced bytes" is not
standing in for "it produced a video":

```
codec_name=h264   profile=High   level=42   pix_fmt=yuv420p
width=1080        height=1920    r_frame_rate=60/1
nb_frames=180     duration=2.983 bit_rate=12799845
```

Decoded frames 60 and 170 show the print at two different heights, with the printed material in
grey and the layer being laid down in gold — so the frame-index animation, the draw range and the
capture are all doing what they claim.

| frame 60                          | frame 170                          |
| --------------------------------- | ---------------------------------- |
| ![frame 60](phase0-frame-060.png) | ![frame 170](phase0-frame-170.png) |

A 30-second Reel is 1800 frames, so this is roughly **7 seconds of export for 30 seconds of
video**, with the renderer in the loop.

**Verdict on Windows: pass.**

### macOS (WKWebView / Safari)

**Not measured. There is no Mac in this environment.**

This is the risk the whole export design rests on, so it stays open until someone runs the
harness on macOS 13+:

```bash
npm install
VITE_BENCH=1 npx tauri dev --release
```

The relevant lines are the ones beginning `[bench] encode:` and `[bench]   probe`. What we need
to know, in order:

1. Is `VideoEncoder` defined at all?
2. Does any `avc1.*` configuration report `supported: true`?
3. Does a timed run produce a non-zero byte count without an error?
4. What encode rate does it reach?

Safari has shipped WebCodecs `VideoEncoder` since 16.4, and macOS 13 is our floor, so the
expectation is that it works. If it does not, plan B is a thin `objc2` binding to VideoToolbox on
the Rust side — still Apple's own encoder, still zero licence surface — and the frame-sequence
export (PNG / WebP) already covers the case where no codec is available at all.

---

## Decisions this confirms

- The IR format and the raw-byte IPC hold up: 22.5 MB crosses as an `ArrayBuffer` and becomes
  GPU buffers with one copy, for one attribute, once per file.
- `setDrawRange` on a single merged geometry is not a compromise; it is the reason the scrub is
  free.
- Driving the animation by frame index rather than wall-clock time costs nothing and is what
  lets the encoder run at 235 fps instead of being paced at 60.
- Nothing here needs FFmpeg, and nothing here needs a bundled codec.

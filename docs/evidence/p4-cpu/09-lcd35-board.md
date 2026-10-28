# EX247, step 2: the Waveshare ESP32-P4-WIFI6-Touch-LCD-3.5 (`p4/lcd35-board`)

Branch commit `fb80eb0f` (the board model, the shared GPIO's pin count, the LEDC's P4 layout,
the FT5x06, the I2S1 pump, merged with `p4/rev3-step1` at `f98f5699`) against `p4/rev3-step1`
at `f98f5699`, the branch it is stacked on, as [README.md](README.md) describes. Results:
[09-lcd35-board.json](09-lcd35-board.json), and the two reruns of the C3 and C6 `hello`
[09-lcd35-board-run2.json](09-lcd35-board-run2.json) and
[09-lcd35-board-run3.json](09-lcd35-board-run3.json).

The other chips' paths change in `esp-periph/src/gpio.rs`: its arrays grow from 49 to 57 entries
(`GPIO_MAX_PINS`), and the pin bounds and masks in `note_out`, `resolve`, `status_now`, `irq`,
`set_input` and the high-word writes read the chip's `pins` field (49 on every chip but the P4)
in place of the constant 49. `esp-periph/src/ledc.rs` gains a `P4` arm in the layout matches.
The FT5x06 is a new device the other boards do not attach. Every run of a workload retired the
same instructions and printed the same console on both builds.

## run 1, started 2026-10-26T23:41:08+0100

Machine: an Apple M5 Max (18 cores) on macOS 27.0.1, not otherwise idle (a test suite had
finished a few minutes before); rustc 1.99.1 (58cf8f9c3 2026-10-13). Both trees extracted with
`git archive` into sibling directories of the same path length and built with
`cargo build --release --bins`. As in step 1, the script ran from a third checkout of the
branch, because the trees from `git archive` have no fetched firmware; both binaries read the
same `web/wasm/fw` files.

| workload | `p4/rev3-step1` median (range) | branch median (range) | change | branch slower | instructions | console SHA-256 | 1-min load before / after |
| --- | ---: | ---: | ---: | ---: | ---: | --- | --- |
| s3-hello | 0.26 (0.26-0.27) | 0.25 (0.25-0.26) | -3.85% | 0/7 | 11,990,305 + 6,798,164 | `04d4912461cb…` | 4.9 / 5.0 |
| c3-hello | 2.38 (2.33-2.41) | 2.41 (2.38-2.51) | +1.26% | 6/7 | 4,800,000,000 | `3b9d8d3eb053…` | 5.0 / 4.3 |
| c6-hello | 1.61 (1.59-1.63) | 1.61 (1.59-1.65) | +0.00% | 4/7 | 4,800,000,000 | `bd69fbb9ac5c…` | 4.3 / 4.0 |
| pocket-tank | 7.25 (7.15-7.41) | 7.18 (7.10-7.24) | -0.97% | 2/7 | 1,115,772,031 + 946,151,715 | `1d1d57f7c150…` | 4.0 / 5.8 |
| c6-wifi-station | 1.07 (1.06-1.09) | 1.07 (1.06-1.09) | +0.00% | 3/7 | 2,240,000,000 | `75b99af7d413…` | 5.8 / 5.3 |

Flag: `c3-hello`, slower in 6 of 7 pairs (+1.26%); the ranges overlap.

## runs 2 and 3, the C3 and C6 `hello` again (2026-10-26T23:44:45 and 23:45:51)

The same binaries and method, straight after run 1, to see whether the flag holds.

| workload | run | `p4/rev3-step1` median (range) | branch median (range) | change | branch slower | 1-min load before / after |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| c3-hello | 2 | 2.43 (2.40-2.45) | 2.42 (2.41-2.53) | -0.41% | 3/7 | 4.9 / 4.5 |
| c6-hello | 2 | 1.64 (1.57-1.67) | 1.62 (1.59-1.65) | -1.22% | 3/7 | 4.5 / 4.4 |
| c3-hello | 3 | 2.39 (2.38-2.42) | 2.39 (2.39-2.46) | +0.00% | 3/7 | 4.4 / 3.7 |
| c6-hello | 3 | 1.60 (1.60-1.65) | 1.63 (1.62-1.71) | +1.87% | 6/7 | 3.7 / 3.4 |

Instructions and console hashes as in run 1. The C3 flag does not hold (3/7 twice), and in
run 3 the flag moves to the C6, which was neutral in runs 1 and 2: noise at this load, about
one or two 10 ms ticks. Not flagged overall.

The JSON's ROM paths were written as `$ESP32SIM_ROM_DIR` before they were committed; nothing
else was changed. SHA-256 as the script wrote them and as committed:

| file | as written | as committed |
| --- | --- | --- |
| `09-lcd35-board.json` | `a928f9fbaae347a308fcd326119093a3f01119f4dad1dd4ed96498337cb191e1` | `d62c874a35d7b22ee46185f9812a9900a5ec1a5a9b658aca8264c52b2fb4d708` |
| `09-lcd35-board-run2.json` | `0778f24833dc810d641bdb7ea9e5b361922b82a682947ac9dfade56fca302e27` | `3d9a7b424e19f7b106751aaa6660639ba58c043867a78ffcae568ade5bd8e54d` |
| `09-lcd35-board-run3.json` | `377ecc79be3568bdf5431d823e2c438f3a99e6fb6c282210e8b2f507268e538b` | `47260b952f61a02dda3275b82be100d83c96b10374451ecffe7b8a3c2821636a` |

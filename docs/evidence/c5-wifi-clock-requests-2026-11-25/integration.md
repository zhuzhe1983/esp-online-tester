# Original C5 Wi-Fi clock requests reach the MAC boundary

The unchanged LCD2.73 factory executes all four native C5 MODEM_SYSCON
requests: `400 → 404 → 604 → 7ff` at `600a9c14`. Its actual OSI caller is
`420e3fb2`; it enters `40814bc8`, then stops at `408142c6` reading private
MAC word `600ad858`. No value is invented for that read. LCD pixels and SD
commands remain0, so original UI/buttons/filesystem/persistence acceptance
is pending.

The [receipt](integration-checks.json) pins sources, inputs, commands and
bounded actual stores. All567 C5 tests pass, including all6 supplied-input
cases, with no failures, ignores or filters. Workspace/all-target and release
WASM/jit-tests Clippy pass with warnings denied. Focused runs use one build
job and release LTO disabled; durations are diagnostic, not CPU benchmarks.
The original reports44,199,490 run steps/CPU instruction count,
44,199,486 nominal cycles and4536 UART bytes. This is not a retired counter.

The [module contract](README.md) implements only the header-backed CLK_CONF1
word and four independent request groups. Applied MODEM_APB permission
qualifies access; a WIFI_APB request cannot provide its own grant. No
effective clock, MAC/PHY/RF engine, calibration result, IRQ or per-tick event
is added. CPU-only reset retains the optional owner, HP/Chip clear it and
raw profile exit drops it. Six parent Bus/guest cases cover provider loss,
actual four-group RMW under plain/UnitCost, unsupported/partial requests,
reset, watchdog precedence and the unimplemented MAC frontier.

The first continuation reached this MAC read as a cause5 access fault; that
negative capture remains pinned. The exact strict word now stops as a
ModelLimit at the same instruction/counts, without returning data or
acknowledging writes. Earlier guest interrupt CSRs remain
`mepc40802bec/mcause80000013/mtval0`. Old-watchdog service still precedes read
or partial-write refusal. Neighboring and off-profile unknown words retain
their previous mapping. This corrects classification of missing hardware;
it does not implement a MAC register.

A separate source audit checked C5 TRM1.1, fixed IDF5.5.3/5.5.4 headers and
ROM exports, and the genuine rev100 ROM. The paired TBTT SoC-wakeup
enable/disable helpers associate slot0 bit29 with this word, without
establishing whole-word POR, access types or readback. The TRM marks the
range Reserved and supplies no MAC field table; the public wdev header
only supplies RNG. These checked sources cannot justify returning0,
treating the word as plain RAM or generating ready. Later MAC/PHY paths
remain static candidates. The receipt pins the sources and audit hash;
it makes no claim about unsearched private or historical specifications.

Independent source-only reviews found no P1/P2 and retained stable
fingerprints; neither prefilled the unified run. No firmware patch, PC
override, function hook, forced peripheral success or host drawing supplies
this continuation. A new default-release production WASM build succeeded
and is separately pinned; its ABI/worker/browser outcomes are not prefilled.
Full workspace CI, CPU controls, current runtime checks and independent
product acceptance remain required. Raw streams and private paths remain
outside Git; numeric facts, historical hashes and negative boundaries stay
intact.

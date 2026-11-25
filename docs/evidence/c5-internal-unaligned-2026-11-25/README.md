# C5 internal unaligned scalar accesses

The unchanged IDF6.0.2 bootloader now passes its packed image-revision reads.
Previously `lhu s2,15(s1)` at4084eeaa, reading408556ab, faulted because C5Bus
required natural alignment for every data load. The official GCC15 esp-base
configuration permits these accesses, while the previous GCC14 build emitted
byte loads for the same packed fields. No firmware or compiler workaround was
applied.

[C5 TRM1.1](https://documentation.espressif.com/esp32-c5_technical_reference_manual_en.pdf)
HP section2.12.1.2, printed p123, describes ordinary unaligned accesses split
into multiple loads/stores. The validated-ROM profile now permits little-endian
16/32-bit scalar data spans wholly inside one internal ROM, HP SRAM or LP SRAM
backing. ROM stays read-only. The original address and complete width still
pass existing APM/PMA/PMP checks before any memory effect. A store uses one
checked slice, so this supported subset does not perform a partial write.

Instruction fetch still requires halfword alignment;32-bit instructions at
PC mod4=2 remain legal. Atomic and DMA guards are unchanged. External/cache,
MMIO and cross-backing unaligned splits stop at an explicit model boundary
before device access. The unselected legacy path is unchanged. This is a
functional memory subset, not a physical split-bus timing, asynchronous error
pipeline or hardware memory-counter implementation. LP counter descriptions
are not evidence for HP counter behavior. The preexisting C5 LR.W misalignment
cause4 versus TRM6 discrepancy remains separate.

Eight owned tests pass, including actual guest LHU instructions, signed loads,
stores, complete-span PMP denial without memory changes, unknown-PMA ModelLimit
without fabricated guest exception state, boundary/IO negatives and fetch/atomic
alignment guards. Tests run both ordinary and priced execution where relevant.
The synthetic backing patterns/header are unit fixtures. They are separate
from the actual unchanged official firmware run. All575 focused C5 tests pass,
including all six available developer-input cases, with0 failures/ignored/
filtered. Both required workspace and release-WASM Clippy checks pass with
warnings denied. No goldens were regenerated. The [receipt](result.json) keeps
three earlier test-fixture/compilation failures and their original log hashes.

The real6.0.2 run advances from522744 to568934 dispatched steps, but then faults
at4084f616 reading6000e058, the temperature-sensor configuration backup in
bootloader_random_enable. It has not reached Key Manager, app_main or Hello.
The new TSENS backup/reset and ADC-divider sequence needs separate modeling;
printing an entropy-enable message does not establish entropy. The earlier
5.5.1 Key Manager boundary also remains unresolved.

The unchanged original LCD2.73 factory still reaches its same MAC ModelLimit:
44199490 run_steps/CPU insn_count,44199486 nominal cycles,4536 UART bytes with
the exact previous UART hash. These are not retired-instruction counts. No
LCD pixels or SD commands are produced. Original UI/buttons/TF acceptance is
still false. Full default-release/latest-main integration, controlled CPU
checks and a current production WASM/worker remain separate verification gates.
Raw logs/artifacts remain in ignored local outputs; compact hashes, commands,
inputs, measured outcomes and limits are retained here.

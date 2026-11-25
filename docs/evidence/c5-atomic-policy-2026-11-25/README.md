# C5 atomic permission probe

RISC-V LR/SC/RMW now asks the Bus for policy before the actual memory operation. The default adds no policy, and the recording and S31 Bus adapters forward it. The probe itself does not read or write device data, create a reservation or invent a priced memory access. A failed SC with no reservation has no memory operation to probe.

The bounded C5 validated-ROM/APM profile accepts supported RAM and ready guest-configured Quad PSRAM atomics with actual constrained CPU context. It rejects unimplemented IO/unknown/alignment requests before effects. Three core tests cover LR/SC/RMW fault-before-effect, failed SC and successful access counts. One C5 test covers4 ordinary/priced UART-negative and RAM-positive scenarios: denied IO preserves RX and emits no TX; RAM changes17to18 and returns17. The priced execution records0 data accesses for rejection and2 for success, catching a recording-wrapper policy omission.

Both warnings-denied Clippy commands pass. The original factory still reaches the same pre-PMA breakpoint with20820883steps/20820882 CPU instruction-count slots/20820864 nominalcycles/2150 UART bytes and no access fault. The initial fixture budget assumption and its correction remain identified in [checks.json](checks.json). All release checks here explicitly use LTO=false.

This is not PMA/PMP integration or a product acceptance result. Native execution stops before420713ac, and final workspace/performance/production-WASM/browser/product checks remain pending. No physical IO-atomic behavior, full memory selftest or app_main/LCD/TF success is claimed.

# Explicit host-debounced Cardputer-Adv keys

`b7224990` adds an optional Rust transport for edges already debounced by a host.
The logical4×14 matrix and physical wiring remain the same. Ordinary matrix input
still requires two25ms scans; browser input does not switch transport automatically.
This batch addresses event delivery and backpressure, with no CPU throughput claim.
Related input-preservation history is EX115/EX116; this transport explicitly changes
the input timing contract for its caller, rather than changing the physical scanner.

`BoardModel` adds two methods at its tail:

- `matrix_key_debounced_at(cycle,row,column,pressed)` returns `Accepted`, `Duplicate`,
  `Full`, `Disabled` or `Invalid` (`repr(u8)` values0–4 respectively). Unsupported boards
  default to `Disabled`. `Full` never overwrites an event or changes TCA state, even
  when the ordinary hardware overflow-overwrite setting is enabled. Duplicate input
  uses no FIFO slot. Interrupt masking permits polling consumers to enqueue normally.
- `matrix_key_diagnostics()` returns an optional copied observation: `accepted:u64`,
  `consumed:u64`, `pending:u8`, `interrupt_pending:bool`; unsupported boards return
  `None`. Counts include successful physical and explicit transport events. Only a
  nonempty KEY_EVENT_A/reg04 read increments `consumed`. The bool observes the enabled,
  latched key-event interrupt: `(INT_STAT & CFG & 1) != 0`.

Both types are re-exported by `esp_soc::board`. Existing wrappers compile with the
defaults but must forward both methods to provide this feature. Rust binary layout
is not a compatibility contract: rebuild the complete core/adapter. No App C ABI or
public WASM export is added.

The first implementation exposed a real integration failure. After reg04 popped an
event and the guest W1C-cleared INT_STAT, the board could still cache GPIO11 as low.
Admitting the next event immediately then hid both the high ACK and the next low edge.
The exact source regression fails on `cce5857f` with `[]`; `5a394b84` synchronizes the
old IRQ before admission and observes high@0, low@10 without an intervening board tick.

That board repair does not complete a consumer handshake. A FIFO pop precedes guest
INT_STAT acknowledgement. A separate integration still reproduced a final pending
event with FIFO-only admission and a fixed delay. `212cb8d6` exposes the read-only ACK
state; it does not impose a new admission gate. The integrating App reported complete
24/24 Wordle and6/6 UserDemo event-register reads at CPI16, and12/12 Wordle reads at
diagnostic CPI64, after waiting for FIFO0 and the ACK, then applying its own1ms guest
cooldown. These are bounded integration observations, not a guarantee for unknown
firmware or proof that an application has processed every popped event.

Native source checks cover all56 logical keys through actual S3 I2C controller
transactions, GPIO11 IRQ, FIFO pressure/retry, masks, reset/reattachment, ordinary
scan timing and the immediate ACK/admission race. TCA11 and board4 tests pass.
On the combined latest public main, both Clippy checks, default-LTO workspace tests
(3,521 reported passing entries,249 summaries),14 production WASM demos, platform,
real worker/recording, ABI and generated-WASM differential checks pass.
The initial all-platform attempt lacked the new C5 ROM in an older cache; it is
retained as a setup failure. The unchanged build passes with the complete verified
ROM set. See [commands, hashes, negative results and limits](results.json).

Reproduce the directed checks:

```sh
cargo test --release -p esp-periph tca8418
cargo test --release -p esp32s3 --test cardputer_adv_keys
cargo clippy -p esp-periph -p esp32s3 --all-targets -- -D warnings
```

FIFO/IRQ observations are not software-ISR completion signals. Keep the normal
physical path for hardware-timing fidelity. A host selecting this nonphysical path
owns edge ordering, retry, held-state reconciliation and consumer-specific pacing.

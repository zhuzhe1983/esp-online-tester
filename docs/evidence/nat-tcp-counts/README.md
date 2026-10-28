# Read-only NAT TCP occupancy

`Nat::tcp_flow_counts(&self) -> TcpFlowCounts` returns current transport occupancy:

- `active`: `Connecting` and `Connected`, including guest half-close, queued writes,
  a host FIN awaiting acknowledgement, and a completed exchange before `poll` moves it
  to `TimeWait`.
- `time_wait`: completed flows retained to re-ACK a retransmitted guest FIN.
- `Closed`: excluded immediately, even if the vector still retains the entry until `poll`.

The snapshot does not poll host sockets or connector channels, expire flows, refresh
activity, retransmit packets, or change cumulative statistics. A connector result that
is ready but not yet polled still counts as active. It is an O(n) query over the
existing bounded TCP table; there are no new fields, tick calls, policy rules, socket
paths or admission limits. UDP is unchanged. No existing CLI calls the query.

## Tests

`esp-soc/src/nat/tests/counts.rs` (eight tests) uses only the existing private injected
connector and loopback socket helpers: empty/failed admission, pending/ready/refused/
disconnected connector, guest reset, mixed states and cumulative-statistics independence,
host write failure, draining and half-closed guest, unacknowledged host FIN, final-ACK
retry, TimeWait timeout and active idle timeout. Repeated snapshots through an immutable
borrow check that pending connector results and protocol bookkeeping stay untouched.

`nice -n 10 cargo +1.99.0 test --release -p esp-soc nat::tests`: 22 passed.

## Static comparison

Base `f1b5c5d5`, head `80d22688`; later commits on this branch change only comments and
this receipt. Build each checkout with `nice -n 10 cargo +1.99.0 build --release --bins`
in separate target directories. For each of `esp32sim`, `esp32sim-c3`, `esp32sim-c6` and
`esp32sim-p4`, disassemble with
`xcrun llvm-objdump --disassemble --demangle --no-show-raw-insn` and run
`python3 docs/evidence/nat-tcp-counts/compare.py base.dis head.dis`.

Each CLI has 2,608 normalized-identical functions, zero different and zero missing or
added functions; raw instruction text also matches. Binary and disassembly hashes are
in `static.json`.

## CPU comparison

No timing run: the method has no callers, and the disassembly is unchanged (static.json).

## Limits

The API exposes counts, not flow identities or layout, and does not change admission or
retention. No firmware or goldens change.

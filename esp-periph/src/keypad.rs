//! Optional host-debounced key transport; callers explicitly choose this instead of matrix scanning.

/// Admission of one already-stable host edge. Full/Disabled/Invalid leave device state unchanged.
/// Duplicate needs no FIFO slot; the host may retire that edge without emitting another event.
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
#[repr(u8)]
pub enum KeyEventAcceptance { Accepted=0, Duplicate=1, Full=2, Disabled=3, Invalid=4 }

/// Read-only FIFO observations since device construction. Accepted counts successful pushes
/// from either physical scans or explicit host transport; consumed counts actual event-register
/// pops, not firmware/application processing. A physical overflow policy may discard old events.
/// interrupt_pending is the enabled, latched key-event IRQ; a FIFO pop does not ACK it.
#[derive(Clone,Copy,Debug,Default,Eq,PartialEq)]
pub struct KeyEventDiagnostics {pub accepted:u64,pub consumed:u64,pub pending:u8,pub interrupt_pending:bool}

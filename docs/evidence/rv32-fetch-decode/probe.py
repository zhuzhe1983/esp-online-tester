"""Insert / remove the post-run retired-instruction report (local probe, never committed).
usage: probe.py apply|revert <tree>"""
import sys
from pathlib import Path
f = Path(sys.argv[2]) / 'cli/src/lib.rs'
s = f.read_text()
A = ('fn run<S: Soc>(mut m: Machine<S>, o: &Opts) {', 'fn run<S: Soc>(mut m: Machine<S>, o: &Opts) where S::Core: \'static {')
B = ('    let dt = t0.elapsed().as_secs_f64();\n    report(&mut m, o, stop, dt);\n',
     '    let dt = t0.elapsed().as_secs_f64();\n    if let Some(cpu) = (&m.cores[0] as &dyn std::any::Any).downcast_ref::<<esp32c6::soc::C6 as Soc>::Core>() { eprintln!("[probe] core0 retired={}", cpu.retired_count); }\n    report(&mut m, o, stop, dt);\n')
if sys.argv[1] == 'apply':
    for a, b in (A, B):
        assert s.count(a) == 1, a
        s = s.replace(a, b)
else:
    for a, b in (A, B):
        assert s.count(b) == 1, b
        s = s.replace(b, a)
f.write_text(s)
print(sys.argv[1], 'ok')

#!/usr/bin/env python3
"""Compare Unit Button native C6 codegen without machine-specific paths.

Usage: python compare-codegen.py REPO BASE.dis ORIGINAL.dis COLD.dis
Disassemble each release binary with:
  xcrun llvm-objdump -d --demangle --no-show-raw-insn BINARY > OUTPUT.dis
Uses the repository's preserved EX220-era address-normalizing comparator.
"""
import pathlib,sys,types,json,hashlib,re
repo=pathlib.Path(sys.argv[1]);m=types.ModuleType('asmcmp')
source=repo/'docs/evidence/classic-core/asmcmp.py'
exec(source.read_text().split('a, b = funcs(sys.argv[1])')[0],m.__dict__)
base,original,cold=[m.funcs(file) for file in sys.argv[2:5]]
selected=re.compile(r'esp32c6::.*(tick|fetch|read|write|refresh|pending|next_deadline)|Machine<esp32c6::soc::C6>>.*(run|step_core|after_round|web_poll_input|web_push_due)')
rows=[]
for name in sorted(set(base)|set(original)|set(cold)):
    if not selected.search(name) or '{{closure}}' in name:continue
    row={'function':name,'baseInstructions':len(base.get(name,[])),'originalInstructions':len(original.get(name,[])),'coldInstructions':len(cold.get(name,[]))}
    row['originalMatchesBase']=name in base and name in original and m.same(base,original,name)
    row['coldMatchesBase']=name in base and name in cold and m.same(base,cold,name)
    rows.append(row)
print(json.dumps({'comparatorSha256':hashlib.sha256(source.read_bytes()).hexdigest(),'normalization':m.__doc__,'selected':rows,'originalDifferences':[r['function'] for r in rows if not r['originalMatchesBase']],'coldDifferences':[r['function'] for r in rows if not r['coldMatchesBase']]},indent=2))

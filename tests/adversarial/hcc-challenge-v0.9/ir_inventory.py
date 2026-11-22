#!/usr/bin/env python3
"""Inventory candidate atomic/TLS IR for manual semantic review; not a correctness gate."""
import argparse, hashlib, json, pathlib, re
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--run-dir',type=pathlib.Path,required=True);p.add_argument('--out',type=pathlib.Path,required=True);a=p.parse_args();root=a.run_dir.resolve();out=a.out.resolve()
if out.exists():p.error('output exists')
results=json.loads((root/'results.json').read_text());
if results['engine']!='hcc':p.error('requires HCC run with emitted IR')
rows=[]
for case in results['results']:
 files=sorted((root/case['id']).glob('native-*.ll'))
 for f in files:
  s=f.read_text();lines=s.splitlines(); matches=[{'line':i,'text':l.strip()} for i,l in enumerate(lines,1) if re.search(r'\b(?:atomicrmw|cmpxchg|fence|thread_local)\b|\b(?:load|store)\s+atomic\b|^target (?:triple|datalayout)',l)]
  rows.append(dict(id=case['id'],file=str(f.relative_to(root)),sha256=hashlib.sha256(f.read_bytes()).hexdigest(),lines=matches))
report=dict(note='Inventory only. Review relevant instructions, types, alignments, ordering and control flow. Stronger documented orderings are allowed; no regex can prove semantics.',compiler_sha256=results['compiler_sha256'],opt=results['opt'],platform=results['platform'],files=rows)
out.parent.mkdir(parents=True,exist_ok=True);out.write_text(json.dumps(report,indent=2)+'\n');print(f'{len(rows)} IR files inventoried; manual review required')

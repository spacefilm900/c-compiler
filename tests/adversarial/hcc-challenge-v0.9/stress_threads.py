#!/usr/bin/env python3
"""Repeat already-built concurrency cases without changing frozen first-run evidence."""
import argparse,concurrent.futures,hashlib,json,os,pathlib,subprocess
R=pathlib.Path(__file__).resolve().parent
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--run-dir',type=pathlib.Path,required=True);p.add_argument('--out',type=pathlib.Path,required=True);p.add_argument('--repetitions',type=int,default=20);p.add_argument('--jobs',type=int,default=2);p.add_argument('--timeout',type=int,default=30);p.add_argument('--case',action='append',help='select stress-marked IDs');a=p.parse_args()
if a.repetitions<1 or a.jobs<1 or a.timeout<1:p.error('positive repetitions/jobs/timeout required')
def sha(f):return hashlib.sha256(f.read_bytes()).hexdigest()
base=a.run_dir.resolve();f=base/'results.json';frozen=json.loads(f.read_text());manifest=R/'manifest.json';m=json.loads(manifest.read_text())
if frozen['manifest_sha256']!=sha(manifest):p.error('first-run manifest differs')
ids={x['id']:x for x in frozen['results']};cs=[x for x in m if x.get('stress',False) and (not a.case or x['id'] in a.case)]
if not cs or (a.case and set(a.case)!={x['id'] for x in cs}):p.error('unknown or non-stress case')
rundir=a.out.resolve();rundir.mkdir(parents=True,exist_ok=False)
exes={};missing=[]
for c in cs:
 exe=base/c['id']/'native.exe'
 if c['id'] not in ids or not ids[c['id']]['checks'].get('native') or not exe.is_file():missing.append(c['id']);continue
 exes[c['id']]=(exe,sha(exe))
jobs=[(c,i) for c in cs if c['id'] in exes for i in range(1,a.repetitions+1)]
def run(j):
 c,i=j;exe,sh=exes[c['id']]
 try:
  z=subprocess.run([str(exe)],cwd=exe.parent,capture_output=True,text=True,errors='replace',timeout=a.timeout,env={**os.environ,'LC_ALL':'C'});rc,so,se=z.returncode,z.stdout,z.stderr
 except subprocess.TimeoutExpired:rc,so,se=None,'','timeout'
 exp=c['expected'];return dict(id=c['id'],repetition=i,exe_sha256=sh,returncode=rc,stdout=so,stderr=se,passed=rc==exp['exit'] and so==exp['stdout'] and se==exp['stderr'])
with concurrent.futures.ThreadPoolExecutor(max_workers=a.jobs) as pool:results=list(pool.map(run,jobs))
r=dict(frozen_results_sha256=sha(f),engine=frozen['engine'],opt=frozen['opt'],compiler_sha256=frozen['compiler_sha256'],repetitions=a.repetitions,unavailable_cases=missing,passed=sum(z['passed'] for z in results),total=len(cs)*a.repetitions,executed=len(results),results=results);(rundir/'results.json').write_text(json.dumps(r,indent=2)+'\n');print(f'stress: {r["passed"]}/{r["total"]}; unavailable {missing}');raise SystemExit(r['passed']!=r['total'])

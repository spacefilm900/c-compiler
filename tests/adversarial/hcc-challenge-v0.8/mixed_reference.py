#!/usr/bin/env python3
"""Reproduce GCC/Clang native mixed-translation-unit oracle checks."""
import argparse,concurrent.futures,json,os,pathlib,platform,shutil,subprocess,sys,hashlib
R=pathlib.Path(__file__).resolve().parent
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--gcc',default='gcc');p.add_argument('--clang',default='clang');p.add_argument('--out',type=pathlib.Path,required=True);p.add_argument('--jobs',type=int,default=4);p.add_argument('--timeout',type=int,default=30);a=p.parse_args()
if sys.platform!='linux' or platform.machine()!='x86_64':p.error('native x86-64 Linux required')
cc={k:shutil.which(v) for k,v in [('gcc',a.gcc),('clang',a.clang)]}
if not all(cc.values()):p.error('missing compiler')
cc={k:str(pathlib.Path(v).resolve()) for k,v in cc.items()};o=a.out.resolve();o.mkdir(parents=True,exist_ok=False)
cs=[c for c in json.loads((R/'manifest.json').read_text()) if len(c['files'])==2 and c['kind']=='run'];jobs=[(c,opt,first) for c in cs for opt in ['O0','O2'] for first in ['gcc','clang']]
def run(cmd,wd):
 try:
  z=subprocess.run(cmd,cwd=wd,capture_output=True,text=True,errors='replace',timeout=a.timeout,env={**os.environ,'LC_ALL':'C'});return dict(cmd=cmd,rc=z.returncode,stdout=z.stdout,stderr=z.stderr)
 except subprocess.TimeoutExpired:return dict(cmd=cmd,rc=None,stdout='',stderr='timeout')
def task(job):
 c,opt,first=job;other='clang' if first=='gcc' else 'gcc';wd=o/(c['id']+'-'+opt+'-'+first);wd.mkdir();logs=[];objs=[]
 for i,f in enumerate(c['files']):
  obj=wd/f'tu{i}.o';logs.append(run([cc[first if i==0 else other],'-std='+c['mode'],*(['-pedantic-errors'] if c['mode']=='c11' else []),'-'+opt,*c['flags'],'-c',str(R/'review-tests'/c['id']/f),'-o',str(obj)],wd));objs.append(obj)
  if logs[-1]['rc']!=0 or not obj.is_file():break
 exe=wd/'test'
 if len(objs)==2 and all(x['rc']==0 for x in logs) and all(x.is_file() for x in objs):logs.append(run([cc['clang'],'-'+opt,*map(str,objs),*c['flags'],'-lm','-o',str(exe)],wd))
 executed=exe.is_file() and logs[-1]['rc']==0
 if executed:logs.append(run([str(exe)],wd))
 exp=c['expected'];passed=executed and logs[-1]['rc']==exp['exit'] and logs[-1]['stdout']==exp['stdout'] and logs[-1]['stderr']==exp['stderr'];res=dict(id=c['id'],opt=opt,first_tu=first,second_tu=other,executed=executed,passed=passed,commands=logs);(wd/'result.json').write_text(json.dumps(res,indent=2)+'\n');return res
with concurrent.futures.ThreadPoolExecutor(max_workers=a.jobs) as pool:results=list(pool.map(task,jobs))
r=dict(platform=platform.platform(),compilers={k:dict(path=v,sha256=hashlib.sha256(pathlib.Path(v).read_bytes()).hexdigest(),version=run([v,'--version'],o)) for k,v in cc.items()},passed=sum(z['passed'] for z in results),total=len(results),results=results);(o/'results.json').write_text(json.dumps(r,indent=2)+'\n');print(f'mixed GCC/Clang: {r["passed"]}/{r["total"]}')
for z in results:
 if not z['passed']:print('FAIL',z['id'],z['opt'],z['first_tu'])
raise SystemExit(r['passed']!=r['total'])

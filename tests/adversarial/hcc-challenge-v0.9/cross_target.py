#!/usr/bin/env python3
"""Freestanding static-assert and target-object checks. Optional matching native Linux execution; no retargeting or emulation."""
import argparse,concurrent.futures,hashlib,json,subprocess,pathlib,shutil,os,re,sys,platform
R=pathlib.Path(__file__).resolve().parent/'cross-target'
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--engine',choices=['clang','hcc'],required=True);p.add_argument('--compiler');p.add_argument('--clang',default='clang');p.add_argument('--opt',choices=['O0','O2'],required=True);p.add_argument('--target',action='append');p.add_argument('--out',required=True,type=pathlib.Path);p.add_argument('--jobs',type=int,default=4);p.add_argument('--timeout',type=int,default=30);p.add_argument('--run-native-linux',action='store_true',help='also link and run matching native Linux cells on a matching host; others remain object-only');a=p.parse_args()
if a.run_native_linux and (sys.platform!='linux' or platform.machine() not in ('x86_64','aarch64','arm64')):p.error('--run-native-linux requires Linux x86-64 or AArch64')
clang=shutil.which(a.clang);cc=shutil.which(a.compiler or ('hcc' if a.engine=='hcc' else a.clang))
if not clang or not cc:p.error('missing tool')
clang=str(pathlib.Path(clang).resolve());cc=str(pathlib.Path(cc).resolve());o=a.out.resolve();o.mkdir(parents=True,exist_ok=False)
cs=[c for c in json.loads((R/'manifest.json').read_text()) if not a.target or c['target'] in a.target]
if not cs or (a.target and set(a.target)!={c['target'] for c in cs}):p.error('unknown target')
def capture(cmd,wd):
 try:
  x=subprocess.run(cmd,cwd=wd,capture_output=True,text=True,errors='replace',timeout=a.timeout,env={**os.environ,'LC_ALL':'C'});return dict(cmd=cmd,rc=x.returncode,stdout=x.stdout,stderr=x.stderr)
 except subprocess.TimeoutExpired:return dict(cmd=cmd,rc=None,stdout='',stderr='timeout')
def family(t):
 arch=t.split('-')[0];arch='arm64' if arch in ('aarch64','arm64') else arch
 osenv='apple' if '-apple-' in t and ('macos' in t or 'darwin' in t) else 'linux-gnu' if 'linux' in t and 'gnu' in t else 'windows-msvc' if 'windows' in t and 'msvc' in t else 'windows-gnu' if ('windows' in t and 'gnu' in t) or 'mingw32' in t else 'unknown'
 return arch,osenv

def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()
def case(c):
 wd=o/c['id'];wd.mkdir();t=c['target_arg'];logs=[];objs=[];ir_info=[];issues=[]
 for i,f in enumerate(c['files']):
  src=R/f;obj=wd/f'test-{i}.o';ll=wd/f'test-{i}.ll'
  if a.engine=='hcc':
   logs.append(capture([cc,'-std=c11','-target',t,str(src),'-o',str(ll)],wd))
   if logs[-1]['rc']!=0 or not ll.is_file():issues.append('frontend failure');break
   text=ll.read_text();tm=re.search(r'^target triple\s*=\s*"([^"]+)"',text,re.M);dm=re.search(r'^target datalayout\s*=\s*"([^"]+)"',text,re.M)
   emitted=tm.group(1) if tm else '';ir_info.append(dict(file=ll.name,sha256=sha(ll),triple=emitted,datalayout=dm.group(1) if dm else None))
   if not emitted or not dm or family(emitted)!=family(t):issues.append('wrong/missing target or data layout');break
   # Assemble for the emitted triple, avoiding a proxy or silently changing target family.
   logs.append(capture([clang,'--target='+emitted,'-'+a.opt,'-c',str(ll),'-o',str(obj)],wd))
  else:logs.append(capture([cc,'--target='+t,'-std=c11','-pedantic-errors','-ffreestanding','-'+a.opt,'-c',str(src),'-o',str(obj)],wd))
  if logs[-1]['rc']!=0 or not obj.is_file():issues.append('object generation failure');break
  if 'overriding the module target triple' in logs[-1]['stderr']:issues.append('triple normalization warning; inspect emitted vs backend triple')
  objs.append(obj)
 object_pass=len(objs)==len(c['files']) and not any('failure' in x or 'missing' in x for x in issues)
 run=None
 if object_pass and a.run_native_linux and family(c['target'])==family(platform.machine()+'-linux-gnu'):
  exe=wd/'test.exe';logs.append(capture([clang,'-'+a.opt,*map(str,objs),'-o',str(exe)],wd))
  if logs[-1]['rc']==0 and exe.is_file():
   logs.append(capture([str(exe)],wd));run=logs[-1]['rc']==c['expected_exit'] and logs[-1]['stdout']=='' and logs[-1]['stderr']==''
  else:run=False
 result={**c,'passed':object_pass and run is not False,'object_pass':object_pass,'runtime_pass':run,'issues':issues,'objects':[dict(file=x.name,sha256=sha(x)) for x in objs],'ir':ir_info,'commands':logs,'evidence':'native execution' if run is not None else 'target object compilation only'}
 (wd/'result.json').write_text(json.dumps(result,indent=2)+'\n');return result
with concurrent.futures.ThreadPoolExecutor(max_workers=a.jobs) as pool:results=list(pool.map(case,cs))
report=dict(engine=a.engine,opt=a.opt,compiler=cc,compiler_sha256=sha(pathlib.Path(cc)),clang=clang,clang_sha256=sha(pathlib.Path(clang)),clang_version=capture([clang,'--version'],o),platform=platform.platform(),passed=sum(x['passed'] for x in results),total=len(results),object_pass=sum(x['object_pass'] for x in results),native_pass=sum(x['runtime_pass'] is True for x in results),native_total=sum(x['runtime_pass'] is not None for x in results),results=results)
(o/'results.json').write_text(json.dumps(report,indent=2)+'\n');print(f'{a.engine} {a.opt}: {report["passed"]}/{report["total"]}; objects {report["object_pass"]}/{report["total"]}; native {report["native_pass"]}/{report["native_total"]}')
for x in results:
 if not x['passed']: print('FAIL',x['id'],x['issues'],x['commands'][-1]['stderr'][:800])
raise SystemExit(report['passed']!=report['total'])

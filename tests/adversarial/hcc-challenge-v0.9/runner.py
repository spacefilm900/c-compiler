#!/usr/bin/env python3
"""Standalone native x86_64 or AArch64 Linux challenge. Never modifies the corpus."""
import argparse, concurrent.futures, hashlib, json, os, pathlib, platform, shutil, subprocess, sys, tempfile, time
ROOT=pathlib.Path(__file__).resolve().parent

def parse():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--engine', choices=['gcc','clang','hcc'], required=True)
    p.add_argument('--compiler', help='path to hcc or reference compiler executable')
    p.add_argument('--linker',default='clang',help='Clang executable for candidate IR and linking')
    p.add_argument('--abi-reference',help='reference C compiler for mixed HCC ABI checks (default: linker Clang)')
    p.add_argument('--opt',choices=['O0','O2'],required=True)
    p.add_argument('--out',required=True,type=pathlib.Path)
    p.add_argument('--jobs',type=int,default=4)
    p.add_argument('--timeout',type=float,default=30)
    p.add_argument('--case',action='append',help='run only specified IDs (partial evidence)')
    return p.parse_args()

def main():
    args=parse()
    if sys.platform!='linux' or platform.machine() not in ('x86_64','AMD64','aarch64','arm64'):
        raise SystemExit('This release runner targets native x86_64 or AArch64 Linux; other targets require a separate protocol.')
    cc=shutil.which(args.compiler or args.engine)
    link=shutil.which(args.linker)
    if not cc or not link: raise SystemExit('compiler/linker not found')
    cc=str(pathlib.Path(cc).resolve()); link=str(pathlib.Path(link).resolve())
    abi=shutil.which(args.abi_reference) if args.abi_reference else link
    if not abi: raise SystemExit('ABI reference not found')
    abi=str(pathlib.Path(abi).resolve())
    out=args.out.resolve();out.mkdir(parents=True,exist_ok=False)
    cases=json.loads((ROOT/'manifest.json').read_text()); selected=[c for c in cases if not args.case or c['id'] in args.case]
    if args.case and set(args.case)!={c['id'] for c in selected}: raise SystemExit('unknown case')
    env=os.environ.copy();env['LC_ALL']='C'
    def capture(cmd,wd):
        t=time.monotonic()
        try:
            p=subprocess.run(cmd,cwd=wd,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=args.timeout)
            return dict(cmd=cmd,returncode=p.returncode,stdout=p.stdout.decode('utf-8','replace'),stderr=p.stderr.decode('utf-8','replace'),seconds=round(time.monotonic()-t,4),timeout=False)
        except subprocess.TimeoutExpired as e:
            return dict(cmd=cmd,returncode=None,stdout=(e.stdout or b'').decode('utf-8','replace'),stderr=(e.stderr or b'').decode('utf-8','replace'),seconds=round(time.monotonic()-t,4),timeout=True)
    def good(p): return p['returncode']==0 and not p['timeout']
    def run_one(c):
        wd=out/c['id'];wd.mkdir(); logs=[]; verdicts={}
        srcs=[ROOT/'review-tests'/c['id']/s for s in c['files']]; mode=c['mode']; flags=c['flags']; opt='-'+args.opt
        def record(cmd):
            z=capture(cmd,wd); logs.append(z); return z
        def compile_obj(src,index,tag,candidate):
            obj=wd/f'{tag}-{index}.o'
            if candidate:
                ir=wd/f'{tag}-{index}.ll'; p=record([cc,'-std='+mode,str(src),'-o',str(ir)])
                if not good(p) or not ir.is_file():return None,'frontend'
                p=record([link,'-w',opt,'-c',str(ir),'-o',str(obj)])
            else:
                ref=abi if args.engine=='hcc' else cc
                strict_flags=['-pedantic-errors'] if mode=='c11' else []
                p=record([ref,'-std='+mode,*strict_flags,opt,*flags,'-c',str(src),'-o',str(obj)])
            return (obj,None) if good(p) and obj.is_file() else (None,'object generation')
        def rejection(mode,tag):
            dest=wd/(tag+('.ll' if args.engine=='hcc' else '.o'))
            cmd=([cc,'-std='+mode,str(srcs[0]),'-o',str(dest)] if args.engine=='hcc' else [cc,'-std='+mode,'-pedantic-errors',opt,'-c',str(srcs[0]),'-o',str(dest)])
            p=record(cmd); err=p['stderr'].lower();
            return (not p['timeout'] and p['returncode'] is not None and p['returncode']>0 and p['returncode']!=101 and (args.engine!='hcc' or (p['returncode']!=2 and 'error:' in err)) and not dest.exists() and bool(err.strip()) and not any(x in err for x in ('internal compiler error','segmentation fault','panicked at','stack overflow','killed signal','please submit a bug report')))
        if c['kind']=='reject': verdicts['required-diagnostic']=rejection(mode,'reject')
        else:
            configurations=[('native',[args.engine=='hcc']*len(srcs))]
            if args.engine=='hcc' and len(srcs)==2:
                configurations += [('mixed-hcc-first',[True,False]),('mixed-hcc-second',[False,True])]
            for tag,choices in configurations:
                objects=[];error=None
                for i,(s,using_candidate) in enumerate(zip(srcs,choices)):
                    obj,error=compile_obj(s,i,tag,using_candidate)
                    if error: break
                    objects.append(str(obj))
                if error: verdicts[tag]=False; continue
                exe=wd/(tag+'.exe'); lnk=link if args.engine=='hcc' else cc
                # Standard math library is always linked; GCC references may use libatomic.
                libs=['-lm']+(['-latomic'] if args.engine=='gcc' else [])
                # HCC documents lock-free codegen without libatomic. Do not mask a missing lowering.
                linkflags=[f for f in flags if not (args.engine=='hcc' and f=='-latomic')]
                p=record([lnk,opt,*objects,*linkflags,*libs,'-o',str(exe)])
                if not good(p) or not exe.is_file(): verdicts[tag]=False;continue
                p=record([str(exe)]); exp=c['expected']; verdicts[tag]=(not p['timeout'] and p['returncode']==exp['exit'] and p['stdout']==exp['stdout'] and p['stderr']==exp['stderr'])
            if c['strict_probe']: verdicts['strict-mode-reject']=rejection('c11','strict')
        result=dict(id=c['id'],title=c['title'],passed=all(verdicts.values()),checks=verdicts,commands=logs)
        (wd/'result.json').write_text(json.dumps(result,indent=2)+'\n'); return result
    with concurrent.futures.ThreadPoolExecutor(max_workers=max(1,args.jobs)) as pool:
        results=list(pool.map(run_one,selected))
    data=dict(engine=args.engine,compiler=cc,compiler_sha256=hashlib.sha256(pathlib.Path(cc).read_bytes()).hexdigest(),linker=link,linker_sha256=hashlib.sha256(pathlib.Path(link).read_bytes()).hexdigest(),abi_reference=abi,abi_reference_sha256=hashlib.sha256(pathlib.Path(abi).read_bytes()).hexdigest(),versions={'compiler':capture([cc,'--help' if args.engine=='hcc' else '--version'],out),'linker':capture([link,'--version'],out),'abi_reference':capture([abi,'--version'],out)},platform=platform.platform(),opt=args.opt,timeout=args.timeout,manifest_sha256=hashlib.sha256((ROOT/'manifest.json').read_bytes()).hexdigest(),passed=sum(r['passed'] for r in results),total=len(results),results=results)
    (out/'results.json').write_text(json.dumps(data,indent=2)+'\n')
    for r in results:
        if not r['passed']: print('FAIL',r['id'],r['title'],r['checks'])
    print(f"{args.engine} {args.opt}: {data['passed']}/{data['total']}")
    return 0 if data['passed']==data['total'] else 1
if __name__=='__main__': raise SystemExit(main())

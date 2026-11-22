#!/usr/bin/env python3
"""Create and preserve deterministic generated workload pairs and use the frozen runner.
Variant A computes the contribution inline; B extracts it into a static helper.
Each independently checks a closed-form, host-computed invariant. No candidate output is an oracle.
"""
import argparse, json, pathlib, shutil, subprocess, sys, hashlib
R=pathlib.Path(__file__).resolve().parent
p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--engine',choices=['hcc','gcc','clang'],required=True);p.add_argument('--compiler');p.add_argument('--linker',default='clang');p.add_argument('--opt',choices=['O0','O2'],required=True);p.add_argument('--seeds',type=int,default=100);p.add_argument('--start-seed',type=int,default=1);p.add_argument('--out',type=pathlib.Path,required=True);p.add_argument('--jobs',type=int,default=4);p.add_argument('--timeout',type=float,default=30);a=p.parse_args()
if not 1<=a.seeds<=10000 or a.start_seed<0:p.error('invalid seed range')
o=a.out.resolve();o.mkdir(parents=True,exist_ok=False);corpus=o/'generated';corpus.mkdir();shutil.copy(R/'runner.py',corpus/'runner.py');m=[]
for seed in range(a.start_seed,a.start_seed+a.seeds):
 # Avoid Python random implementation dependence: a fixed arithmetic generator.
 n=(2,4,8,16,32)[seed%5]; rounds=64+(seed*37%193); align=(8,16,32,64)[(seed//5)%4]; factor=1+(seed*17%13); bias=seed*13%97
 expected=sum((i*factor+bias+j) for i in range(n) for j in range(rounds))
 for variant in ('inline','helper'):
  id=f'GEN-{seed:05d}-{variant}';d=corpus/'review-tests'/id;d.mkdir(parents=True)
  helper='static unsigned contribution(unsigned id,unsigned j){return id*F+B+j;}' if variant=='helper' else ''
  expr='contribution(id,j)' if variant=='helper' else '(id*F+B+j)'
  src=f'''#include <threads.h>
#include <stdatomic.h>
#include <stdint.h>
#include <stdlib.h>
#include <stdio.h>
#define CK(x) do {{if(!(x))exit(1);}}while(0)
enum{{N={n},R={rounds},F={factor},B={bias},A={align}}};
struct S{{unsigned count:9;unsigned :0;_Alignas(A) unsigned sum;}};
static _Thread_local struct S scratch;
static atomic_uint total;
static struct S results[N];
{helper}
static int worker(void *arg){{unsigned id=*(unsigned*)arg;CK(scratch.count==0 && scratch.sum==0);CK((uintptr_t)&scratch%A==0);unsigned len=R;unsigned values[len];for(unsigned j=0;j<len;j++)values[j]={expr};for(unsigned j=0;j<len;j++){{scratch.sum+=values[j];scratch.count++;}}results[id]=scratch;atomic_fetch_add_explicit(&total,scratch.sum,memory_order_relaxed);return 0;}}
int main(void){{thrd_t t[N];unsigned ids[N];for(unsigned i=0;i<N;i++){{ids[i]=i;CK(thrd_create(&t[i],worker,&ids[i])==thrd_success);}}for(unsigned i=0;i<N;i++){{CK(thrd_join(t[i],0)==thrd_success);CK(results[i].count==R);CK(results[i].sum==R*(i*F+B)+R*(R-1)/2);}}CK(total=={expected}u);CK(scratch.count==0 && scratch.sum==0);puts("ok");return 0;}}
'''
  (d/'test.c').write_text(src)
  m.append(dict(id=id,group='generated',title=f'seed {seed} {variant}',seed=seed,variant=variant,parameters=dict(threads=n,rounds=rounds,alignment=align,factor=factor,bias=bias,expected=expected),kind='run',mode='c11',files=['test.c'],flags=['-pthread'],strict_probe=False,expected=dict(compile='accept',stdout='ok\n',stderr='',exit=0)))
(corpus/'manifest.json').write_text(json.dumps(m,indent=2)+'\n')
(corpus/'SHA256SUMS.txt').write_text(''.join(f'{hashlib.sha256(f.read_bytes()).hexdigest()}  {f.relative_to(corpus)}\n' for f in sorted(corpus.rglob('*')) if f.is_file() and f.name!='SHA256SUMS.txt'))
cmd=[sys.executable,str(corpus/'runner.py'),'--engine',a.engine,'--linker',a.linker,'--opt',a.opt,'--out',str(o/'run'),'--jobs',str(a.jobs),'--timeout',str(a.timeout)]
if a.compiler:cmd+=['--compiler',a.compiler]
rc=subprocess.run(cmd).returncode
if not (o/'run'/'results.json').is_file():raise SystemExit(rc or 1)
r=json.loads((o/'run'/'results.json').read_text());by={x['id']:x for x in r['results']};pairs=[]
for seed in range(a.start_seed,a.start_seed+a.seeds):
 ids=[f'GEN-{seed:05d}-{v}' for v in ('inline','helper')];checks=[by[i]['passed'] for i in ids];pairs.append(dict(seed=seed,ids=ids,passed=all(checks),variant_results=checks))
report=dict(engine=a.engine,opt=a.opt,total_pairs=len(pairs),passed_pairs=sum(x['passed'] for x in pairs),pairs=pairs,raw_results='run/results.json');(o/'pairs.json').write_text(json.dumps(report,indent=2)+'\n'); print(f'paired workload invariants: {report["passed_pairs"]}/{report["total_pairs"]}');raise SystemExit(rc or int(report['passed_pairs']!=report['total_pairs']))

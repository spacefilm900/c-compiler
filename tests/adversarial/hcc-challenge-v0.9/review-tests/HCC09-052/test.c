#include <stdio.h>
#include <stdlib.h>
#define CK(x) do { if (!(x)) exit(1); } while (0)
#define OK() do { puts("ok"); return 0; } while (0)
#include <threads.h>
#include <stdatomic.h>
#include <time.h>
#define JOIN(t) CK(thrd_join(t,0)==thrd_success)
#define START(t,f,a) CK(thrd_create(&(t),f,a)==thrd_success)
struct Job{int n,row;int *out;};int f(void*p){struct Job*j=p;int n=j->n;int a[n][n];for(int i=0;i<n;i++)for(int k=0;k<n;k++)a[i][k]=i+k;int sum=0;for(int k=0;k<n;k++)sum+=a[j->row][k];*j->out=sum;return 0;}int main(void){int out[4];struct Job j[4];thrd_t t[4];for(int i=0;i<4;i++){j[i]=(struct Job){4,i,&out[i]};START(t[i],f,&j[i]);}for(int i=0;i<4;i++){JOIN(t[i]);CK(out[i]==4*i+6);}OK();}

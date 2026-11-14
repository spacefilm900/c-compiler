/* Labels as values and computed goto (GCC manual, "Labels as Values"):
   `&&label` is the address of a label, of type `void *`, and `goto *expr;`
   jumps to it. Lua 5.4's interpreter loop (lvm.c with ljumptab.h) builds a
   static table of label addresses inside the function and dispatches with
   `goto *disptab[op];`, which this test models (written out, without
   Lua's macros). GNU C only.

   Label addresses are only jumped to or compared with null here: the LLVM
   Language Reference makes an equality test between two label addresses
   (`blockaddress` values) undefined, so this compiler does not promise it. */

int printf(const char *, ...);

enum { OP_PUSH, OP_ADD, OP_MUL, OP_DUP, OP_JNZ, OP_DEC, OP_SWAP, OP_ACC, OP_HALT, NUM_OPS };

/* A static, const table of label addresses inside the function, as Lua
   writes it. */
static long run(const int *code) {
    static const void *const disptab[NUM_OPS] = {
        &&L_OP_PUSH, &&L_OP_ADD, &&L_OP_MUL, &&L_OP_DUP,
        &&L_OP_JNZ, &&L_OP_DEC, &&L_OP_SWAP, &&L_OP_ACC, &&L_OP_HALT,
    };
    long stack[16];
    int sp = 0;
    int pc = 0;
    goto *disptab[code[pc]];
    L_OP_PUSH: {
        stack[sp++] = code[pc + 1];
        pc += 2;
        goto *disptab[code[pc]];
    }
    L_OP_ADD: {
        sp--;
        stack[sp - 1] += stack[sp];
        pc++;
        goto *disptab[code[pc]];
    }
    L_OP_MUL: {
        sp--;
        stack[sp - 1] *= stack[sp];
        pc++;
        goto *disptab[code[pc]];
    }
    L_OP_DUP: {
        stack[sp] = stack[sp - 1];
        sp++;
        pc++;
        goto *disptab[code[pc]];
    }
    L_OP_JNZ: {
        if (stack[--sp] != 0)
            pc = code[pc + 1];
        else
            pc += 2;
        goto *disptab[code[pc]];
    }
    L_OP_DEC: {
        stack[sp - 1]--;
        pc++;
        goto *disptab[code[pc]];
    }
    L_OP_SWAP: {
        long t = stack[sp - 1];
        stack[sp - 1] = stack[sp - 2];
        stack[sp - 2] = t;
        pc++;
        goto *disptab[code[pc]];
    }
    L_OP_ACC: {
        stack[sp - 2] += stack[sp - 1];
        pc++;
        goto *disptab[code[pc]];
    }
    L_OP_HALT: {
        return stack[sp - 1];
    }
}

/* An automatic table, and label addresses in variables. */
static int classify(int n) {
    void *targets[3] = {&&zero, &&small, &&big};
    void *p = n == 0 ? targets[0] : n < 10 ? targets[1] : targets[2];
    goto *p;
zero:
    return 100;
small:
    return 200 + n;
big:
    return 300;
}

/* A label reached only through the computed goto, after the jump. */
static int only_indirect(int k) {
    void *where = k ? &&yes : &&no;
    int r = 1;
    goto *where;
    r = 99; /* unreachable */
yes:
    r += 10;
no:
    return r;
}

/* A table of `void *` (not const), a recursive caller, and a cast. */
static int depth(int n) {
    static void *steps[] = {&&down, &&done};
    goto *(void *)steps[n == 0];
down:
    return 1 + depth(n - 1);
done:
    return 0;
}

/* A label address is not a null pointer (comparison with null is
   defined). */
static int not_null(void) {
    void *p = &&mark;
    if (p != 0)
        goto *p;
    return 0;
mark:
    return 1;
}

int main(void) {
    static const int prog[] = {OP_PUSH, 6, OP_PUSH, 7, OP_MUL, OP_PUSH, 8, OP_ADD, OP_HALT};
    printf("prog = %ld\n", run(prog));
    /* total = 0; n = 100; while (n) { total += n; n--; } */
    static const int loop[] = {
        OP_PUSH, 0, OP_PUSH, 100, /* 0, 2: total n */
        OP_DUP, OP_JNZ, 9,        /* 4, 5 */
        OP_SWAP, OP_HALT,         /* 7, 8: return total */
        OP_ACC, OP_DEC,           /* 9, 10 */
        OP_PUSH, 1, OP_JNZ, 4,    /* 11, 13: jump back */
    };
    printf("loop = %ld\n", run(loop));
    for (int i = 0; i < 12; i += 5)
        printf("classify(%d) = %d\n", i, classify(i));
    printf("only_indirect = %d %d\n", only_indirect(1), only_indirect(0));
    printf("depth = %d\n", depth(7));
    printf("not null = %d\n", not_null());
    return 0;
}

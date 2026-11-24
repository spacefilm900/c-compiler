/* GNU C: `enum E` named before the definition of E (C11 6.7.2.3p3 forbids
   it). QuickJS has `typedef enum OPCodeEnum OPCodeEnum;` ahead of the
   enumerator list and uses the typedef only after it. */
int printf(const char *, ...);

typedef enum Op Op;
static int name_len(Op op);

enum Op { OP_NOP, OP_PUSH, OP_POP, OP_COUNT };

static int name_len(Op op) { return op == OP_PUSH ? 4 : 3; }

struct Insn {
    Op op : 2;
    unsigned char arg;
};

int main(void) {
    struct Insn code[3] = { { OP_PUSH, 7 }, { OP_POP, 0 }, { 3, 1 } };
    Op last = OP_COUNT;
    printf("%d %d %d %d %zu\n", name_len(code[0].op), code[1].op, code[2].op, last - 4 > 0,
           sizeof(Op));
    return 0;
}

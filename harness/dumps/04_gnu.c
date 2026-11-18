// dump-flags: -std=gnu11
/* Golden dump test: GNU extensions (statement expressions, the ?: with an
   omitted operand, case ranges and typeof), in -std=gnu11. */
int classify(int c) {
    switch (c) {
    case 'a' ... 'z':
        return 1;
    case '0' ... '9':
        return 2;
    }
    return 0;
}

int main(void) {
    __typeof__(1 + 2L) wide = 3;
    int y = ({ int t = (int)wide; t * 2; });
    int z = 0 ?: y;
    return classify('q') + classify('5') - 3 + (z - 6);
}

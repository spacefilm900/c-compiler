int side;

enum E { EA = 2, EB = 7 };

int fall(int x) {
    int n = 0;
    switch (x) {
    case 1:
        n = 1;
        break;
    case 2:
    case 3:
        n = 2;
    case 4:
        n = n + 4;
        break;
    default:
        n = 9;
        break;
    }
    return n;
}

int nest(int a, int b) {
    switch (a) {
    case 1:
        switch (b) {
        case 1:
            return 11;
        default:
            break;
        }
        return 12;
    case 2:
        return 20;
    default:
        return 30;
    }
}

int cont(void) {
    int s = 0;
    int i = 0;
    while (i < 4) {
        switch (i) {
        case 1:
            i = i + 1;
            continue;
        default:
            s = s + i;
            break;
        }
        i = i + 1;
    }
    return s;
}

int duff(int n) {
    int sum = 0;
    int count = n;
    switch (count % 4) {
    case 0:
        do {
            sum = sum + 1;
            case 3:
                sum = sum + 1;
            case 2:
                sum = sum + 1;
            case 1:
                sum = sum + 1;
                count = count - 4;
        } while (count > 0);
    }
    return sum;
}

int buried(int x) {
    int n = 0;
    int y = 0;
    switch (x) {
    case 1:
        if ((y = 1)) {
            case 2:
                n = 2;
        } else {
            n = 1;
        }
        n = n + y * 10;
        break;
    default:
        n = 4;
        break;
    }
    return n;
}

int skip_init(int x) {
    int r = 0;
    switch (x) {
    case 0:
        side = side + 1;
        int y = 5;
        r = y;
        break;
    case 1:
        y = 9;
        r = y;
        break;
    default:
        r = 7;
        break;
    }
    return r;
}

int on_char(char c) {
    switch (c) {
    case -1:
        return 1;
    case 2:
        return 2;
    case 356:
        return 3;
    default:
        return 4;
    }
}

int on_uchar(unsigned char c) {
    switch (c) {
    case 255:
        return 1;
    case -1:
        return 2;
    default:
        return 3;
    }
}

int on_short(short s) {
    switch (s) {
    case -2:
        return 1;
    case 40000:
        return 2;
    default:
        return 3;
    }
}

int on_ushort(unsigned short s) {
    switch (s) {
    case 65535:
        return 1;
    default:
        return 2;
    }
}

int on_uint(unsigned int c) {
    switch (c) {
    case -1:
        return 1;
    case 2u:
        return 2;
    default:
        return 3;
    }
}

int on_long(long c) {
    switch (c) {
    case -1:
        return 1;
    case 8589934592L:
        return 2;
    default:
        return 3;
    }
}

int on_ulong(unsigned long c) {
    switch (c) {
    case -1:
        return 1;
    default:
        return 2;
    }
}

int on_ll(long long c) {
    switch (c) {
    case -1LL:
        return 1;
    case 2LL:
        return 2;
    default:
        return 3;
    }
}

int on_ull(unsigned long long c) {
    switch (c) {
    case -1:
        return 1;
    case 2ull:
        return 2;
    default:
        return 3;
    }
}

int on_bool(_Bool b) {
    switch (b) {
    case 0:
        return 1;
    case 1:
        return 2;
    default:
        return 3;
    }
}

int on_const(int c) {
    int dummy = 0;
    switch (c) {
    case EA:
        return 1;
    case 'A':
        return 2;
    case sizeof(int) * 3:
        return 3;
    case (short)-2:
        return 4;
    case (int)sizeof(1, dummy):
        return 5;
    case EB:
        return 6;
    default:
        return 7;
    }
}

int empty(int x) {
    switch (x++) {
    }
    switch (x++)
        ;
    return x;
}

int none(int x) {
    switch (x) {
    case 2:
        return 2;
    }
    return 1;
}

int static_skip(int x) {
    switch (x) {
    case 1:;
        static int s = 10;
        s = 0;
    case 2:
        return s;
    default:
        return 3;
    }
}

int loop_break(void) {
    int cond = 10;
    switch (cond) {
    case 10:
        for (int i = 0; i < 5; i = i + 1) {
            cond = cond - 1;
            if (cond == 8)
                break;
        }
        return cond;
    default:
        return 0;
    }
}

int main(void) {
    if (fall(1) != 1 || fall(2) != 6 || fall(3) != 6 || fall(4) != 4 || fall(5) != 9)
        return 1;
    if (nest(1, 1) != 11 || nest(1, 2) != 12 || nest(2, 0) != 20 || nest(0, 0) != 30)
        return 2;
    if (cont() != 5)
        return 3;
    if (duff(1) != 1 || duff(2) != 2 || duff(4) != 4 || duff(5) != 5 || duff(0) != 4)
        return 4;
    if (buried(1) != 12 || buried(2) != 2 || buried(0) != 4)
        return 5;
    side = 0;
    if (skip_init(0) != 5 || side != 1)
        return 6;
    if (skip_init(1) != 9 || side != 1)
        return 7;
    if (skip_init(3) != 7 || side != 1)
        return 8;
    if (on_char(-1) != 1 || on_char(2) != 2 || on_char(0) != 4)
        return 9;
    if (on_uchar(255) != 1 || on_uchar(0) != 3)
        return 10;
    if (on_short(-2) != 1 || on_short(0) != 3)
        return 11;
    if (on_ushort(65535) != 1 || on_ushort(1) != 2)
        return 12;
    if (on_uint(4294967295u) != 1 || on_uint(2) != 2 || on_uint(0) != 3)
        return 13;
    if (on_long(-1) != 1 || on_long(8589934592L) != 2 || on_long(0) != 3)
        return 14;
    if (on_ulong(18446744073709551615ul) != 1 || on_ulong(1) != 2)
        return 15;
    if (on_ll(-1) != 1 || on_ll(2) != 2 || on_ll(0) != 3)
        return 16;
    if (on_ull(18446744073709551615ull) != 1 || on_ull(2) != 2 || on_ull(0) != 3)
        return 17;
    if (on_bool(0) != 1 || on_bool(1) != 2)
        return 18;
    if (on_const(EA) != 1 || on_const('A') != 2 || on_const(12) != 3)
        return 19;
    if (on_const(-2) != 4 || on_const(4) != 5 || on_const(EB) != 6 || on_const(0) != 7)
        return 20;
    if (empty(3) != 5 || none(1) != 1 || none(2) != 2)
        return 21;
    if (static_skip(2) != 10 || static_skip(1) != 0)
        return 22;
    if (loop_break() != 8)
        return 23;
    return 0;
}

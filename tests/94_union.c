union Num {
    int x;
    unsigned char bytes[4];
};

void copy_num(union Num *out, union Num *p) { out->x = p->x; }

int main(void) {
    union Num u = {0x01020304};
    union Num *p = &u;
    union Num copy;
    if (u.bytes[0] != 0x04) {
        return 1;
    }
    if (u.bytes[3] != 0x01) {
        return 2;
    }
    p->bytes[0] = 7;
    copy_num(&copy, p);
    if (copy.bytes[0] != 7) {
        return 3;
    }
    return copy.x == u.x ? 0 : 4;
}

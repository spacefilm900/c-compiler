int main(void) {
    if (sizeof(1) != 4)
        return 1;
    if (sizeof(1u) != 4 || sizeof(1U) != 4)
        return 2;
    if (sizeof(1l) != 8 || sizeof(1L) != 8)
        return 3;
    if (sizeof(1ll) != 8 || sizeof(1LL) != 8)
        return 4;
    if (sizeof(1ul) != 8 || sizeof(1uL) != 8 || sizeof(1LU) != 8)
        return 5;
    if (sizeof(1ull) != 8 || sizeof(1ULL) != 8 || sizeof(1llu) != 8)
        return 6;
    if (sizeof(2147483648) != 8)
        return 7;
    if (sizeof(0x80000000) != 4)
        return 8;
    if (sizeof(0x100000000) != 8)
        return 9;
    if (sizeof(0x8000000000000000) != 8)
        return 10;
    if (sizeof(4294967296) != 8)
        return 11;
    if (sizeof(4294967295u) != 4)
        return 12;
    if (sizeof(4294967296u) != 8)
        return 13;
    if (!(1u - 2u > 1u))
        return 14;
    if ((1 - 2) > 0)
        return 15;
    if (sizeof(010) != 4)
        return 16;
    if (010 != 8)
        return 17;
    return 0;
}

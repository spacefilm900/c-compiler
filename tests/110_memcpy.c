int printf(const char *, ...);

int main(void) {
    char src[] = "hello";
    char dst[8];
    char *back = __builtin_memcpy(dst, src, 6);
    if (back != dst || dst[0] != 'h' || dst[4] != 'o' || dst[5] != 0) {
        return 1;
    }
    __builtin_memset(dst, 'Z', 4);
    if (dst[0] != 'Z' || dst[3] != 'Z' || dst[4] != 'o') {
        return 2;
    }
    __builtin_memmove(dst + 1, dst, 4);
    printf("%c%c%c%c%c\n", dst[0], dst[1], dst[2], dst[3], dst[4]);
    int nums[4] = {1, 2, 3, 4};
    int copy[4];
    __builtin_memcpy(copy, nums, sizeof nums);
    printf("%d %d %d %d\n", copy[0], copy[1], copy[2], copy[3]);
    return 0;
}

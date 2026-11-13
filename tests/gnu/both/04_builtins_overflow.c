/* Checked arithmetic builtins (GCC manual, "Built-in Functions to Perform
   Arithmetic with Overflow Checking"). The operation is done in infinite
   precision and the result is stored modulo the result type. */

int printf(const char *, ...);

#define SHOW(op, a, b, T, fmt) do { T r; int o = op(a, b, &r); printf(#op " " fmt " %d\n", r, o); } while (0)

int main(void)
{
    SHOW(__builtin_add_overflow, 2147483647, 1, int, "%d");
    SHOW(__builtin_add_overflow, 2147483647, 1, long, "%ld");
    SHOW(__builtin_add_overflow, 200, 100, unsigned char, "%u");
    SHOW(__builtin_add_overflow, -1, 0, unsigned, "%u");
    SHOW(__builtin_add_overflow, -5, 10u, int, "%d");
    SHOW(__builtin_add_overflow, 9223372036854775807L, 1, long, "%ld");
    SHOW(__builtin_add_overflow, 18446744073709551615UL, 1, unsigned long, "%lu");
    SHOW(__builtin_add_overflow, 18446744073709551615UL, -1, long, "%ld");
    SHOW(__builtin_sub_overflow, 0, 1, unsigned, "%u");
    SHOW(__builtin_sub_overflow, -2147483647 - 1, 1, int, "%d");
    SHOW(__builtin_sub_overflow, 5u, 7u, int, "%d");
    SHOW(__builtin_sub_overflow, -9223372036854775807L - 1, 1, long long, "%lld");
    SHOW(__builtin_sub_overflow, 0, -9223372036854775807L - 1, unsigned long, "%lu");
    SHOW(__builtin_mul_overflow, 65536, 65536, int, "%d");
    SHOW(__builtin_mul_overflow, 65536, 65536, long, "%ld");
    SHOW(__builtin_mul_overflow, -1, -1, unsigned, "%u");
    SHOW(__builtin_mul_overflow, -3, 4, int, "%d");
    SHOW(__builtin_mul_overflow, -3, 4, unsigned, "%u");
    SHOW(__builtin_mul_overflow, 4294967296L, 4294967296L, long, "%ld");
    SHOW(__builtin_mul_overflow, 18446744073709551615UL, 18446744073709551615UL, unsigned long, "%lu");
    SHOW(__builtin_mul_overflow, -9223372036854775807L - 1, -1, long, "%ld");
    SHOW(__builtin_mul_overflow, -9223372036854775807L - 1, -1, unsigned long, "%lu");
    SHOW(__builtin_mul_overflow, 3037000500L, 3037000500L, long, "%ld");
    SHOW(__builtin_mul_overflow, (short)-300, (short)200, short, "%d");
    SHOW(__builtin_mul_overflow, 0, -9223372036854775807L - 1, unsigned char, "%u");
    int si; unsigned ui; long sl; unsigned long ul; long long sll; unsigned long long ull;
    printf("%d ", __builtin_sadd_overflow(2147483647, 1, &si)); printf("%d\n", si);
    printf("%d ", __builtin_uadd_overflow(4294967295u, 2u, &ui)); printf("%u\n", ui);
    printf("%d ", __builtin_ssubl_overflow(-9223372036854775807L, 5L, &sl)); printf("%ld\n", sl);
    printf("%d ", __builtin_usubl_overflow(1ul, 2ul, &ul)); printf("%lu\n", ul);
    printf("%d ", __builtin_smulll_overflow(3037000500LL, 3037000500LL, &sll)); printf("%lld\n", sll);
    printf("%d ", __builtin_umulll_overflow(1ull << 32, 1ull << 31, &ull)); printf("%llu\n", ull);
    printf("%d ", __builtin_saddl_overflow(1L, 2L, &sl)); printf("%ld\n", sl);
    printf("%d ", __builtin_uaddll_overflow(1ull, 2ull, &ull)); printf("%llu\n", ull);
    printf("%d ", __builtin_ssub_overflow(1, 2, &si)); printf("%d\n", si);
    printf("%d ", __builtin_usub_overflow(3u, 2u, &ui)); printf("%u\n", ui);
    printf("%d ", __builtin_smul_overflow(-65536, 32768, &si)); printf("%d\n", si);
    printf("%d ", __builtin_umul_overflow(65536u, 65536u, &ui)); printf("%u\n", ui);
    printf("%d ", __builtin_smull_overflow(2L, 3L, &sl)); printf("%ld\n", sl);
    printf("%d ", __builtin_umull_overflow(2ul, 3ul, &ul)); printf("%lu\n", ul);
    printf("%d ", __builtin_saddll_overflow(1LL, 2LL, &sll)); printf("%lld\n", sll);
    printf("%d ", __builtin_ssubll_overflow(1LL, 2LL, &sll)); printf("%lld\n", sll);
    printf("%d ", __builtin_usubll_overflow(1ull, 2ull, &ull)); printf("%llu\n", ull);
    printf("%d ", __builtin_uaddl_overflow(1ul, 2ul, &ul)); printf("%lu\n", ul);
    printf("%d ", __builtin_ssubl_overflow(1L, 2L, &sl)); printf("%ld\n", sl);
    return 0;
}

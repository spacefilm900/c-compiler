/* N1570 7.17-7.30 stdatomic.h, stdio.h, stdlib.h, string.h, tgmath.h, threads.h, time.h, uchar.h, wchar.h, wctype.h */
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <tgmath.h>
#include <threads.h>
#include <time.h>
#include <uchar.h>
#include <wchar.h>
#include <wctype.h>
#include "check.h"

static atomic_int shared = ATOMIC_VAR_INIT(0);
static int worker(void *arg) {
    for (int i = 0; i < 1000; i++)
        atomic_fetch_add_explicit(&shared, 1, memory_order_relaxed);
    return *(int *)arg;
}
static int cmp(const void *a, const void *b) { return *(const int *)a - *(const int *)b; }
static once_flag once = ONCE_FLAG_INIT;
static int once_count;
static void init_once(void) { once_count++; }

int main(void) {
    /* stdatomic.h */
    atomic_flag flag = ATOMIC_FLAG_INIT;
    CHECK(!atomic_flag_test_and_set(&flag) && atomic_flag_test_and_set(&flag));
    _Atomic long al = 5;
    long expected = 5;
    CHECK(atomic_compare_exchange_strong(&al, &expected, 9) && atomic_load(&al) == 9);
    CHECK(atomic_exchange(&al, 1) == 9 && atomic_is_lock_free(&al));
    atomic_thread_fence(memory_order_seq_cst);
    CHECK(ATOMIC_INT_LOCK_FREE >= 1 && kill_dependency(3) == 3);
    /* threads.h */
    thrd_t t[4];
    int ids[4] = { 1, 2, 3, 4 }, sum = 0;
    for (int i = 0; i < 4; i++) CHECK(thrd_create(&t[i], worker, &ids[i]) == thrd_success);
    for (int i = 0; i < 4; i++) { int res; thrd_join(t[i], &res); sum += res; }
    CHECK(sum == 10 && atomic_load(&shared) == 4000);
    mtx_t m;
    CHECK(mtx_init(&m, mtx_plain) == thrd_success && mtx_lock(&m) == thrd_success && mtx_unlock(&m) == thrd_success);
    mtx_destroy(&m);
    call_once(&once, init_once);
    call_once(&once, init_once);
    CHECK(once_count == 1);
    /* stdio.h */
    char buf[64];
    CHECK(snprintf(buf, sizeof buf, "%5.2f|%-3d|%s|%c|%x|%o|%e", 3.14159, 7, "s", 'c', 255, 8, 12345.678) > 0);
    puts(buf);
    FILE *f = tmpfile();
    CHECK(f != 0);
    fprintf(f, "%d %s\n", 42, "word");
    rewind(f);
    int n; char w[8];
    CHECK(fscanf(f, "%d %7s", &n, w) == 2 && n == 42 && strcmp(w, "word") == 0);
    fclose(f);
    CHECK(EOF < 0 && BUFSIZ >= 256 && FOPEN_MAX >= 8);
    /* stdlib.h */
    int arr[5] = { 5, 3, 1, 4, 2 };
    qsort(arr, 5, sizeof *arr, cmp);
    int key = 4;
    CHECK(arr[0] == 1 && arr[4] == 5 && *(int *)bsearch(&key, arr, 5, sizeof *arr, cmp) == 4);
    CHECK(atoi("12") == 12 && strtod("2.5", 0) == 2.5 && abs(-3) == 3 && labs(-3L) == 3 && llabs(-3LL) == 3);
    div_t d = div(7, 2);
    CHECK(d.quot == 3 && d.rem == 1 && MB_CUR_MAX >= 1 && RAND_MAX >= 32767);
    void *p = aligned_alloc(64, 128);
    CHECK(p && ((unsigned long)p & 63) == 0);
    free(p);
    /* string.h */
    char s[16] = "hello";
    strcat(s, " w");
    CHECK(strlen(s) == 7 && strcmp(s, "hello w") == 0 && strncmp(s, "help", 3) == 0 && strchr(s, 'w') == s + 6);
    CHECK(memcmp("abc", "abd", 3) < 0 && strstr(s, "lo") == s + 3 && strspn("aab", "a") == 2 && strrchr(s, 'l') == s + 3);
    memset(s, 'x', 3); memmove(s + 1, s, 4);
    CHECK(s[0] == 'x' && s[3] == 'x');
    /* tgmath.h: the macro picks the function from the argument type */
    CHECK(_Generic(sqrt(2.0f), float: 1, default: 0) && _Generic(sqrt(2.0), double: 1, default: 0));
    CHECK(_Generic(fabs(2.0L), long double: 1, default: 0) && _Generic(pow(2.0f, 2), double: 1, default: 0));
    CHECK(sqrt(4.0f) == 2.0f && _Generic(sqrt(4), double: 1, default: 0));
    /* time.h */
    time_t now = time(0);
    CHECK(now != (time_t)-1 && difftime(now, now) == 0.0 && CLOCKS_PER_SEC > 0);
    struct tm tm0 = { .tm_year = 100, .tm_mon = 0, .tm_mday = 1 };
    strftime(buf, sizeof buf, "%Y-%m-%d", &tm0);
    CHECK(strcmp(buf, "2000-01-01") == 0);
    struct timespec ts;
    CHECK(timespec_get(&ts, TIME_UTC) == TIME_UTC);
    /* uchar.h, wchar.h, wctype.h */
    char16_t c16 = u'\u00e9'; char32_t c32 = U'\U0001F600';
    CHECK(c16 == 0xe9 && c32 == 0x1F600);
    mbstate_t st;
    memset(&st, 0, sizeof st);
    char32_t out32;
    CHECK(mbrtoc32(&out32, "A", 1, &st) == 1 && out32 == 'A');
    wchar_t ws[16] = L"wide";
    CHECK(wcslen(ws) == 4 && wcscmp(ws, L"wide") == 0 && iswalpha(L'a') && towupper(L'a') == L'A');
    swprintf(ws, 16, L"%d", 12);
    CHECK(wcscmp(ws, L"12") == 0 && WEOF != 0);
    return DONE();
}

/* __DATE__ and __TIME__ (C11 6.10.8.1) are string literals of the forms
   "Mmm dd yyyy" (day padded with a space) and "hh:mm:ss". The values
   change from run to run, so this checks their shape;
   harness/run_tests.py compares the values with Clang's under a fixed
   SOURCE_DATE_EPOCH. */
int printf(const char *, ...);

static int digit(char c) { return c >= '0' && c <= '9'; }

static int same3(const char *a, const char *b) { return a[0] == b[0] && a[1] == b[1] && a[2] == b[2]; }

int main(void)
{
    static const char date[] = __DATE__;
    static const char time[] = __TIME__;
    const char *months = "JanFebMarAprMayJunJulAugSepOctNovDec";
    int month = 0;
    for (int i = 0; i < 12; i++)
        if (same3(date, months + 3 * i))
            month = i + 1;
    int date_ok = month && date[3] == ' ' && (date[4] == ' ' || digit(date[4]))
        && digit(date[5]) && date[6] == ' ' && digit(date[7]) && digit(date[10]);
    int time_ok = digit(time[0]) && digit(time[1]) && time[2] == ':'
        && digit(time[3]) && time[5] == ':' && digit(time[7]);
    printf("%d %d %d %d\n", (int)sizeof date, (int)sizeof time, date_ok, time_ok);
    return !(date_ok && time_ok);
}

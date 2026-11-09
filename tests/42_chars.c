int main(void) {
    if ('a' != 97)
        return 1;
    if ('\n' != 10)
        return 2;
    if ('\0' != 0)
        return 3;
    if ('\x41' != 65)
        return 4;
    if ('\101' != 65)
        return 5;
    if ('\a' != 7 || '\b' != 8 || '\f' != 12)
        return 6;
    if ('\r' != 13 || '\t' != 9 || '\v' != 11)
        return 7;
    if ('\\' != 92 || '\'' != 39 || '\"' != 34 || '\?' != 63)
        return 8;
    if (' ' != 32)
        return 9;
    if (sizeof('a') != 4)
        return 10;
    /* char is signed, so an escape above 127 converts to a negative int. */
    if ('\377' != -1 || '\xff' != -1 || '\x0' != 0)
        return 11;
    if ('\12' != 10)
        return 12;
    return 0;
}

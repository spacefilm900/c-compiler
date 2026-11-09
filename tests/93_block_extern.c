int shared = 5;
static int secret = 1;

int read_secret(void) {
    extern int secret;
    return secret;
}

int shadow(void) {
    int shared = 3;
    {
        extern int shared;
        if (shared != 5)
            return 100;
        shared = 4;
    }
    return shared;
}

int main(void) {
    int extern secret;
    extern int later;
    if (shadow() != 3)
        return 1;
    if (shared != 4)
        return 2;
    if (read_secret() != 1)
        return 3;
    secret = 9;
    if (read_secret() != 9)
        return 4;
    later = later + 1;
    return later - 8;
}

int later = 7;

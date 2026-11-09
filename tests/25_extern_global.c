extern int g;
extern int g;
extern int bias = 1;

int read(void) {
    return g + bias;
}

int g = 6;

int main(void) {
    g = g + 1;
    return read(); /* 8 */
}

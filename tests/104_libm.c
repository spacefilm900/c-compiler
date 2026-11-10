double sin(double);
double cos(double);
double sqrt(double);
double fabs(double);
double fma(double, double, double);
double ldexp(double, int);

int main(void) {
    if (fabs(sin(0.0)) > 1e-9) {
        return 1;
    }
    if (fabs(cos(0.0) - 1.0) > 1e-9) {
        return 2;
    }
    if (fabs(sqrt(4.0) - 2.0) > 1e-9) {
        return 3;
    }
    if (fabs(fma(2.0, 3.0, 0.5) - 6.5) > 1e-9) {
        return 4;
    }
    if (fabs(ldexp(1.5, 3) - 12.0) > 1e-9) {
        return 5;
    }
    return 0;
}

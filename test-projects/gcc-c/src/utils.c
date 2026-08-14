#include <stdio.h>

int add(int a, int b) {
    int temp = a + b;  // warning: unused variable 'temp'
    return a + b;
}

void print_value(double v) {
    printf("%f\n", v);
}

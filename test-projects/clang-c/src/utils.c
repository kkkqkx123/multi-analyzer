#include <stdio.h>

int multiply(int a, int b) {
    int product = a * b;  // warning: unused variable 'product'
    return a * b;
}

void log_msg(const char* msg) {
    printf("%s\n", msg);
}

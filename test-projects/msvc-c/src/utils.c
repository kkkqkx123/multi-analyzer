// msvc-c utils
#include <stdio.h>

int sum(int a, int b) {
    int total = a + b;  // warning C4189: 'total': local variable is initialized but not referenced
    return a + b;
}

void show(double d) {
    printf("%f\n", d);
}

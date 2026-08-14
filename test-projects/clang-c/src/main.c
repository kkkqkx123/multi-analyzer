#include <stdio.h>

int main(void) {
    char* p = 0xdeadbeef;  // error: incompatible integer to pointer conversion
    int unused;            // warning: unused variable 'unused'
    printf("%p\n", (void*)p);
    return 0;
}

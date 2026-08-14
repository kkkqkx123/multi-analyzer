// msvc-c project - C sources for MSVC-style analysis
#include <stdio.h>

int main(void) {
    int value = "string literal";  // error C2440: 'initializing': cannot convert from 'const char [15]' to 'int'
    int unused;                     // warning C4101: 'unused': unreferenced local variable
    printf("%d\n", value);
    return 0;
}

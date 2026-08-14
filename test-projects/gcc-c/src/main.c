#include <stdio.h>

int main(void) {
    int x = "hello";  // error: initialization of 'int' from 'char *' makes integer from pointer
    int unused;       // warning: unused variable 'unused'
    printf("%d\n", x);
    return 0;
}

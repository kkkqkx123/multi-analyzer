// utils.cpp - deliberately contains warnings
int add(int a, int b) {
    int temp = a + b;  // warning: unused variable 'temp'
    int ignored = 0;   // warning: unused variable 'ignored'
    return a + b;
}

// unused parameter warning
double scale(double value, double factor, int mode) {  // warning: unused parameter 'mode'
    return value * factor;
}

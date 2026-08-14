// msvc-cpp project - C++ sources for MSVC-style analysis
#include <iostream>
#include <string>

// MSVC: error C2440 - cannot convert from 'int' to 'std::string'
std::string convert(int n) {
    return n;  // error C2440: 'return': cannot convert from 'int' to 'std::string'
}

int main() {
    int unused_value;  // warning C4101: 'unused_value': unreferenced local variable
    std::cout << convert(3) << std::endl;
    return 0;
}

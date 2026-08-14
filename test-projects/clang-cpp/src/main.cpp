#include <iostream>
#include <vector>
#include <string>

// clang-specific error: no viable conversion
std::string describe(int n) {
    std::vector<int> data;
    data.push_back(n);
    return data[0];  // error: no viable conversion from 'int' to 'std::string'
}

int main() {
    int unused_var;  // warning: unused variable 'unused_var'
    std::cout << describe(7) << std::endl;
    return 0;
}

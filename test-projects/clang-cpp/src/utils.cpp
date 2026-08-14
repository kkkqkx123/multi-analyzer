// clang-cpp utils - warnings only
#include <string>

std::string concat(const std::string& a, const std::string& b, int flag) {  // warning: unused parameter 'flag'
    std::string result = a + b;  // warning: unused variable 'result'
    return a + b;
}

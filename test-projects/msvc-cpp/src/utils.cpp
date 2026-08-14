// msvc-cpp utils - warnings only
#include <string>

std::string join(const std::string& a, const std::string& b, int sep) {  // warning C4100: 'sep': unreferenced formal parameter
    std::string combined = a + b;  // warning C4189: 'combined': local variable is initialized but not referenced
    return a + b;
}

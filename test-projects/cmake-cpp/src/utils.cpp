#include <string>

// Intentional warning: unused parameter
void process_string(const std::string& input, int unused_param) {
    // Intentional warning: unused variable
    int temp = 0;
    // Intentional error: type mismatch
    std::string s = 123;
}

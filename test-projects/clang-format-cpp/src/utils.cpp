#include <string>

// Formatting violations: class brace placement, 2-space indent,
// missing spaces around operators and after commas.
class Greeter{
public:
  std::string greet(const std::string& name){
    std::string msg="Hello, "+name+"!";
    return msg;
  }
};

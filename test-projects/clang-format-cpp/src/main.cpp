#include <iostream>
#include <string>
#include <vector>

// Formatting violations: function brace on same line, no space after comma,
// 2-space indent instead of configured 4, missing spaces around operators.
int add(int a,int b){
  return a+b;
}

int main(){
  std::vector<std::string> names={"alice","bob"};
  for(const auto& n:names){
    std::cout<<n<<std::endl;
  }
  int x=add(1,2);
  std::cout<<x<<std::endl;
  return 0;
}

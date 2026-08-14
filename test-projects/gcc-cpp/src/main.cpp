#include <iostream>
#include <vector>
#include <string>

// int -> string conversion error (types do not match)
int compute(int n) {
    std::vector<int> data;
    data.push_back(n);
    std::string s = data[0];  // error: conversion from 'int' to non-scalar type 'std::string'
    return s.size();
}

int main() {
    int unused;  // warning: unused variable 'unused'
    std::cout << compute(42) << std::endl;
    int x = 10;  // warning: variable 'x' set but not used
    return 0;
}

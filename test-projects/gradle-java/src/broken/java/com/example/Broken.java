package com.example;

// Intentional compile errors for analyzer verification:
// 1. class name does not match file name (should be Broken.java)
// 2. undefined variable
// 3. undefined method
public class Broken {
    public void brokenMethod() {
        // Error: undefined variable
        undefinedVar = 10;

        // Error: undefined method
        undefinedMethod();
    }
}

cmake_minimum_required(VERSION 3.15)
project(CMakeCppTest CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Executable with intentional errors for analyzer verification
add_executable(cmakedemo
    src/main.cpp
    src/utils.cpp
    src/math.cpp
)

target_include_directories(cmakedemo PRIVATE include)

if(MSVC)
    target_compile_options(cmakedemo PRIVATE /W4)
else()
    target_compile_options(cmakedemo PRIVATE -Wall -Wextra)
endif()

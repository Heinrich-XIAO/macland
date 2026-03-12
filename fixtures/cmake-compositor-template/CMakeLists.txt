cmake_minimum_required(VERSION 3.20)
project(cmake_compositor C)

enable_testing()

add_executable(cmake-compositor src/main.c)
target_compile_definitions(cmake-compositor PRIVATE MACLAND_DEFAULT_MODE="unset")

add_test(NAME cmake-compositor-self-test COMMAND cmake-compositor --self-test)


cmake_minimum_required(VERSION 4.3)
project(desktop_qt LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 23)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

find_package(Qt6 REQUIRED COMPONENTS Concurrent Widgets)
find_package(Threads REQUIRED)

if (WIN32)
    set(SLY_CARGO_HINTS "$ENV{USERPROFILE}/.cargo/bin")
else ()
    set(SLY_CARGO_HINTS "$ENV{HOME}/.cargo/bin" "$ENV{CARGO_HOME}/bin")
endif ()
find_program(CARGO_EXECUTABLE cargo HINTS ${SLY_CARGO_HINTS} REQUIRED)

get_filename_component(SLY_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/../.." ABSOLUTE)
set(SLY_TARGET_DIR "${SLY_ROOT}/target")
set(SLY_CARGO_PROFILE "debug" CACHE STRING "Cargo profile for sly-cxx (debug or release)")
set_property(CACHE SLY_CARGO_PROFILE PROPERTY STRINGS debug release)

if (SLY_CARGO_PROFILE STREQUAL "release")
    set(SLY_CARGO_PROFILE_ARGS --release)
elseif (SLY_CARGO_PROFILE STREQUAL "debug")
    set(SLY_CARGO_PROFILE_ARGS)
else ()
    message(FATAL_ERROR "SLY_CARGO_PROFILE must be debug or release")
endif ()

if (MSVC)
    set(SLY_CXX_LIB "${SLY_TARGET_DIR}/${SLY_CARGO_PROFILE}/sly_cxx.lib")
else ()
    set(SLY_CXX_LIB "${SLY_TARGET_DIR}/${SLY_CARGO_PROFILE}/libsly_cxx.a")
endif ()

add_custom_target(sly_cxx_cargo
        COMMAND "${CMAKE_COMMAND}" -E env "CARGO_TARGET_DIR=${SLY_TARGET_DIR}"
        "${CARGO_EXECUTABLE}" build -p sly-cxx ${SLY_CARGO_PROFILE_ARGS}
        WORKING_DIRECTORY "${SLY_ROOT}"
        BYPRODUCTS "${SLY_CXX_LIB}"
        USES_TERMINAL
        COMMENT "Building sly-cxx with Cargo"
)

add_library(sly_cxx STATIC IMPORTED GLOBAL)
set_target_properties(sly_cxx PROPERTIES
        IMPORTED_LOCATION "${SLY_CXX_LIB}"
        INTERFACE_INCLUDE_DIRECTORIES "${SLY_ROOT}/bindings/cxx/include;${SLY_TARGET_DIR}/cxxbridge"
)
target_compile_features(sly_cxx INTERFACE cxx_std_20)
add_dependencies(sly_cxx sly_cxx_cargo)
add_library(sly::cxx ALIAS sly_cxx)

if (WIN32)
    target_link_libraries(sly_cxx INTERFACE ws2_32 userenv bcrypt ntdll)
elseif (NOT APPLE)
    target_link_libraries(sly_cxx INTERFACE Threads::Threads ${CMAKE_DL_LIBS} m)
endif ()

add_executable(desktop_qt
        main.cpp
        main_window.cpp
        main_window.h
        rom_list_model.cpp
        rom_list_model.h
)
set_target_properties(desktop_qt PROPERTIES CXX_SCAN_FOR_MODULES OFF)
target_link_libraries(desktop_qt
        Qt::Concurrent
        Qt::Widgets
        sly::cxx
)

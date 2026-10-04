cmake_minimum_required(VERSION 4.3)

if (CMAKE_HOST_SYSTEM_NAME STREQUAL "Darwin" AND NOT CMAKE_OSX_DEPLOYMENT_TARGET)
    set(CMAKE_OSX_DEPLOYMENT_TARGET "14.0" CACHE STRING "")
endif ()

project(desktop_qt LANGUAGES CXX)

include(GNUInstallDirs)

set(CMAKE_CXX_STANDARD 23)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

find_package(Qt6 REQUIRED COMPONENTS Concurrent GuiPrivate Multimedia ShaderTools Widgets)
find_package(Threads REQUIRED)
qt_standard_project_setup()

option(SLY_ENABLE_GAMEPAD "Enable gamepad support via SDL3" ON)
if (SLY_ENABLE_GAMEPAD)
    find_package(SDL3 3.2 REQUIRED)
endif ()

if (WIN32)
    set(SLY_CARGO_HINTS "$ENV{USERPROFILE}/.cargo/bin")
else ()
    set(SLY_CARGO_HINTS "$ENV{HOME}/.cargo/bin" "$ENV{CARGO_HOME}/bin")
endif ()
find_program(CARGO_EXECUTABLE cargo HINTS ${SLY_CARGO_HINTS} REQUIRED)

get_filename_component(SLY_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/../.." ABSOLUTE)
include("${SLY_ROOT}/cmake/SlyWgslShaders.cmake")

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

file(MAKE_DIRECTORY "${SLY_TARGET_DIR}/cxxbridge")

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

qt_add_executable(desktop_qt
        WIN32
        MACOSX_BUNDLE
        gamepad_input.cpp
        gamepad_input.h
        main.cpp
        main_window.cpp
        main_window.h
        player_window.cpp
        player_window.h
        rom_list_model.cpp
        rom_list_model.h
        video_widget.cpp
        video_widget.h
)

file(GLOB_RECURSE DESKTOP_QT_WGSL_SHADERS CONFIGURE_DEPENDS
        "${CMAKE_CURRENT_SOURCE_DIR}/shaders/*.vert.wgsl"
        "${CMAKE_CURRENT_SOURCE_DIR}/shaders/*.frag.wgsl"
        "${CMAKE_CURRENT_SOURCE_DIR}/shaders/*.comp.wgsl"
)
sly_target_wgsl_shaders(desktop_qt
        BASE_DIR "${CMAKE_CURRENT_SOURCE_DIR}/shaders"
        PREFIX "/shaders"
        FILES ${DESKTOP_QT_WGSL_SHADERS}
)

set_target_properties(desktop_qt PROPERTIES
        OUTPUT_NAME sly-classic
        CXX_SCAN_FOR_MODULES OFF
)
target_link_libraries(desktop_qt
        PRIVATE
        Qt::Concurrent
        Qt::GuiPrivate
        Qt::Multimedia
        Qt::Widgets
        sly::cxx
)

if (SLY_ENABLE_GAMEPAD)
    target_link_libraries(desktop_qt PRIVATE SDL3::SDL3)
endif ()
target_compile_definitions(desktop_qt PRIVATE SLY_HAS_SDL3=$<BOOL:${SLY_ENABLE_GAMEPAD}>)

install(TARGETS desktop_qt
        BUNDLE DESTINATION .
        RUNTIME DESTINATION "${CMAKE_INSTALL_BINDIR}"
)

qt_generate_deploy_app_script(
        TARGET desktop_qt
        OUTPUT_SCRIPT deploy_script
        NO_UNSUPPORTED_PLATFORM_ERROR
)
install(SCRIPT "${deploy_script}")

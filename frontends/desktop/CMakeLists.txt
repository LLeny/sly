cmake_minimum_required(VERSION 3.16)

project(desktop VERSION 0.1.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 23)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

set(CMAKE_EXPORT_COMPILE_COMMANDS ON)
set(QT_QML_GENERATE_QMLLS_INI ON)

find_package(Qt6 6.11.1 REQUIRED COMPONENTS Quick QuickControls2 GuiPrivate)
find_package(Threads REQUIRED)

if(WIN32)
    set(SLY_CARGO_HINTS "$ENV{USERPROFILE}/.cargo/bin")
else()
    set(SLY_CARGO_HINTS "$ENV{HOME}/.cargo/bin" "$ENV{CARGO_HOME}/bin")
endif()
find_program(CARGO_EXECUTABLE cargo HINTS ${SLY_CARGO_HINTS} REQUIRED)

qt_standard_project_setup(REQUIRES 6.11)

get_filename_component(SLY_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/../.." ABSOLUTE)
set(SLY_TARGET_DIR "${SLY_ROOT}/target")
set(SLY_CARGO_PROFILE "debug" CACHE STRING "Cargo profile for sly-cxx (debug or release)")
set_property(CACHE SLY_CARGO_PROFILE PROPERTY STRINGS debug release)

if(SLY_CARGO_PROFILE STREQUAL "release")
    set(SLY_CARGO_PROFILE_ARGS --release)
elseif(SLY_CARGO_PROFILE STREQUAL "debug")
    set(SLY_CARGO_PROFILE_ARGS)
else()
    message(FATAL_ERROR "SLY_CARGO_PROFILE must be debug or release")
endif()

if(MSVC)
    set(SLY_CXX_LIB "${SLY_TARGET_DIR}/${SLY_CARGO_PROFILE}/sly_cxx.lib")
else()
    set(SLY_CXX_LIB "${SLY_TARGET_DIR}/${SLY_CARGO_PROFILE}/libsly_cxx.a")
endif()

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
    INTERFACE_INCLUDE_DIRECTORIES "${SLY_TARGET_DIR}/cxxbridge"
)
add_dependencies(sly_cxx sly_cxx_cargo)
add_library(sly::cxx ALIAS sly_cxx)

if(WIN32)
    target_link_libraries(sly_cxx INTERFACE ws2_32 userenv bcrypt ntdll)
elseif(NOT APPLE)
    target_link_libraries(sly_cxx INTERFACE Threads::Threads ${CMAKE_DL_LIBS} m)
endif()

qt_add_executable(appdesktop
    main.cpp
    player.cpp
    player.h
    viewport.cpp
    viewport.h
)

qt_add_qml_module(appdesktop
    URI desktop
    QML_FILES
        Main.qml
        Player.qml
)

target_compile_definitions(appdesktop PRIVATE SLY_ROMS_DIR="${SLY_ROOT}/roms")

set_target_properties(appdesktop PROPERTIES
#    MACOSX_BUNDLE_GUI_IDENTIFIER com.example.appdesktop
    MACOSX_BUNDLE_BUNDLE_VERSION ${PROJECT_VERSION}
    MACOSX_BUNDLE_SHORT_VERSION_STRING ${PROJECT_VERSION_MAJOR}.${PROJECT_VERSION_MINOR}
    MACOSX_BUNDLE TRUE
    WIN32_EXECUTABLE TRUE
)

target_link_libraries(appdesktop
    PRIVATE
        Qt6::Quick
        Qt6::QuickControls2
        Qt6::GuiPrivate
        sly::cxx
)

install(TARGETS appdesktop
    BUNDLE DESTINATION .
    LIBRARY DESTINATION ${CMAKE_INSTALL_LIBDIR}
    RUNTIME DESTINATION ${CMAKE_INSTALL_BINDIR}
)

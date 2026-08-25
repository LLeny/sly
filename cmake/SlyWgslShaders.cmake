include_guard(GLOBAL)

include(CMakeParseArguments)

set(SLY_NAGA_VERSION "29.0.4" CACHE STRING "Required naga-cli version")

function(sly_target_wgsl_shaders target)
    set(one_value_args BASE_DIR PREFIX)
    set(multi_value_args FILES)
    cmake_parse_arguments(PARSE_ARGV 1 WGSL "" "${one_value_args}" "${multi_value_args}")

    if (WGSL_UNPARSED_ARGUMENTS)
        message(FATAL_ERROR "Unknown arguments to sly_target_wgsl_shaders: ${WGSL_UNPARSED_ARGUMENTS}")
    endif ()
    if (NOT TARGET "${target}")
        message(FATAL_ERROR "sly_target_wgsl_shaders target does not exist: ${target}")
    endif ()
    if (NOT WGSL_FILES)
        return()
    endif ()
    if (NOT TARGET Qt6::qsb)
        message(FATAL_ERROR "sly_target_wgsl_shaders requires the Qt6 ShaderTools component")
    endif ()

    if (NOT WGSL_BASE_DIR)
        set(WGSL_BASE_DIR "${CMAKE_CURRENT_SOURCE_DIR}")
    endif ()
    if (NOT WGSL_PREFIX)
        set(WGSL_PREFIX "/shaders")
    endif ()

    cmake_path(ABSOLUTE_PATH WGSL_BASE_DIR NORMALIZE OUTPUT_VARIABLE wgsl_base_dir)
    find_program(SLY_NAGA_EXECUTABLE NAMES naga DOC "Path to the naga-cli executable")
    if (NOT SLY_NAGA_EXECUTABLE)
        message(FATAL_ERROR
                "naga-cli ${SLY_NAGA_VERSION} is required to build WGSL shaders. "
                "Install it with: cargo install naga-cli --version ${SLY_NAGA_VERSION} --locked")
    endif ()

    execute_process(
            COMMAND "${SLY_NAGA_EXECUTABLE}" --version
            RESULT_VARIABLE naga_version_result
            OUTPUT_VARIABLE naga_version_stdout
            ERROR_VARIABLE naga_version_stderr
            OUTPUT_STRIP_TRAILING_WHITESPACE
            ERROR_STRIP_TRAILING_WHITESPACE
    )
    string(CONCAT naga_version_output "${naga_version_stdout}" " " "${naga_version_stderr}")
    string(REGEX MATCH "[0-9]+\\.[0-9]+\\.[0-9]+" naga_version "${naga_version_output}")
    if (NOT naga_version_result EQUAL 0 OR NOT naga_version STREQUAL SLY_NAGA_VERSION)
        message(FATAL_ERROR
                "Expected naga-cli ${SLY_NAGA_VERSION}, found '${naga_version_output}' at "
                "${SLY_NAGA_EXECUTABLE}")
    endif ()

    set(generated_root "${CMAKE_CURRENT_BINARY_DIR}/sly-wgsl/${target}")
    set(shader_pack_root "${generated_root}/pack")
    set(shader_packs)

    foreach (shader IN LISTS WGSL_FILES)
        cmake_path(ABSOLUTE_PATH shader BASE_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}" NORMALIZE
                OUTPUT_VARIABLE shader_source)
        cmake_path(RELATIVE_PATH shader_source BASE_DIRECTORY "${wgsl_base_dir}"
                OUTPUT_VARIABLE shader_relative)
        if (shader_relative MATCHES "^\\.\\.")
            message(FATAL_ERROR "WGSL shader is outside BASE_DIR: ${shader_source}")
        endif ()
        if (NOT shader_relative MATCHES "\\.(vert|frag|comp)\\.wgsl$")
            message(FATAL_ERROR
                    "WGSL shader names must end in .vert.wgsl, .frag.wgsl, or .comp.wgsl: "
                    "${shader_source}")
        endif ()

        string(REGEX REPLACE "\\.wgsl$" "" shader_glsl_relative "${shader_relative}")
        string(REGEX MATCH "\\.(vert|frag|comp)$" shader_stage_match "${shader_glsl_relative}")
        set(shader_stage "${CMAKE_MATCH_1}")
        set(naga_glsl "${generated_root}/naga/${shader_glsl_relative}")
        set(shader_glsl "${generated_root}/qsb/${shader_glsl_relative}")
        set(shader_pack "${shader_pack_root}/${shader_glsl_relative}.qsb")
        cmake_path(GET naga_glsl PARENT_PATH naga_output_dir)
        cmake_path(GET shader_glsl PARENT_PATH shader_output_dir)
        cmake_path(GET shader_pack PARENT_PATH shader_pack_dir)

        add_custom_command(
                OUTPUT "${shader_pack}"
                COMMAND "${CMAKE_COMMAND}" -E make_directory
                        "${naga_output_dir}"
                        "${shader_output_dir}"
                        "${shader_pack_dir}"
                COMMAND "${SLY_NAGA_EXECUTABLE}"
                        --profile core450
                        --keep-coordinate-space
                        --shader-stage "${shader_stage}"
                        "${shader_source}"
                        "${naga_glsl}"
                COMMAND "${CMAKE_COMMAND}"
                        "-DINPUT=${naga_glsl}"
                        "-DOUTPUT=${shader_glsl}"
                        -P "${CMAKE_CURRENT_FUNCTION_LIST_DIR}/SlyNagaGlslToVulkan.cmake"
                COMMAND "$<TARGET_FILE:Qt6::qsb>"
                        --glsl "300es,330"
                        --hlsl 50
                        --msl 12
                        --orig-file "${shader_source}"
                        --output "${shader_pack}"
                        "${shader_glsl}"
                DEPENDS
                        "${shader_source}"
                        "${CMAKE_CURRENT_FUNCTION_LIST_DIR}/SlyNagaGlslToVulkan.cmake"
                        Qt6::qsb
                COMMENT "Baking WGSL shader ${shader_relative}"
                VERBATIM
        )
        list(APPEND shader_packs "${shader_pack}")
    endforeach ()

    set_source_files_properties(${shader_packs} PROPERTIES GENERATED TRUE)
    qt_add_resources("${target}" "${target}_wgsl_shaders"
            PREFIX "${WGSL_PREFIX}"
            BASE "${shader_pack_root}"
            FILES ${shader_packs}
    )
endfunction()

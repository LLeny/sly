if (NOT DEFINED INPUT OR NOT DEFINED OUTPUT)
    message(FATAL_ERROR "SlyNagaGlslToVulkan.cmake requires INPUT and OUTPUT")
endif ()

file(READ "${INPUT}" shader_source)

string(REPLACE "gl_VertexID" "gl_VertexIndex" shader_source "${shader_source}")
string(REPLACE "gl_InstanceID" "gl_InstanceIndex" shader_source "${shader_source}")

string(REGEX REPLACE
        "uniform ([A-Za-z0-9_]*sampler[A-Za-z0-9_]*) (_group_[0-9]+_binding_([0-9]+)_[A-Za-z0-9_]+);"
        "layout(binding = \\3) uniform \\1 \\2;"
        shader_source
        "${shader_source}"
)

file(WRITE "${OUTPUT}" "${shader_source}")

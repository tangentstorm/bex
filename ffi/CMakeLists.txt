cmake_minimum_required(VERSION 3.15)

project(bex-ffi CXX)

include(FetchContent)
FetchContent_Declare(
  corrosion
  GIT_REPOSITORY https://github.com/corrosion-rs/corrosion.git
  GIT_TAG v0.5.0)
FetchContent_MakeAvailable(corrosion)

find_program(CBINDGEN_EXECUTABLE
    NAMES cbindgen
    DOC "Requires cbindgen to generate the bex headers"
    REQUIRED)

set(bex_capi_include ${PROJECT_BINARY_DIR}/include)
set(bex_capi_h ${bex_capi_include}/bex.h)

add_custom_command(
    OUTPUT ${bex_capi_h}
    COMMAND ${CBINDGEN_EXECUTABLE}
    ARGS --output ${bex_capi_h} ${PROJECT_SOURCE_DIR}
    DEPENDS ${PROJECT_SOURCE_DIR}/src/lib.rs ${PROJECT_SOURCE_DIR}/cbindgen.toml
    COMMENT "Generating bex.h using cbindgen"
    VERBATIM)

add_custom_target(bex-capi-header ALL DEPENDS ${bex_capi_h})

corrosion_import_crate(MANIFEST_PATH Cargo.toml CRATES bex-ffi)

add_dependencies(bex_ffi bex-capi-header)
target_include_directories(bex_ffi INTERFACE ${bex_capi_include})

add_library(bex ALIAS bex_ffi)
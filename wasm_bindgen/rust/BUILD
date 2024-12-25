load("@rules_rust_bindgen//:defs.bzl", "rust_bindgen_toolchain")
load("@bazel_skylib//:bzl_library.bzl", "bzl_library")
load("@rules_rust//crate_universe:defs.bzl", "crate", "crates_vendor")

rust_bindgen_toolchain(
    name = "wasm_bindgen_toolchain_impl",
    bindgen = "@wasm_bindgen//:wasm_bindgen_cli",
)

toolchain(
    name = "wasm_bindgen_toolchain",
    toolchain = "wasm_bindgen_toolchain_impl",
    toolchain_type = "@rules_rust_wasm_bindgen//:toolchain_type",
)

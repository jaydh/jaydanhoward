load("@rules_rust_bindgen//:defs.bzl", "rust_bindgen_toolchain")
load("@bazel_skylib//:bzl_library.bzl", "bzl_library")
load("@rules_rust//crate_universe:defs.bzl", "crate", "crates_vendor")

exports_files([
    "BUILD.wasm-bindgen-cli.bazel",
])

crates_vendor(
    name = "crates_vendor",
    annotations = {
        "log": [crate.annotation(
            rustc_flags = [
                "--cfg=atomic_cas",
                "--cfg=use_std",
            ],
            version = "<5",
        )],
        "proc-macro2": [crate.annotation(
            rustc_flags = ["--cfg=use_proc_macro"],
        )],
        "unicase": [crate.annotation(
            rustc_flags = [
                "--cfg=__unicase__iter_cmp",
                "--cfg=__unicase__defauler_hasher",
            ],
        )],
    },
    mode = "remote",
    packages = {
        "wasm-bindgen": crate.spec(
            version = "0.2.99",
        ),
    },
    # Shorten the repository name to avoid MAX_PATH issues on Windows
    # `rules_rust_wasm_bindgen_deps` == `rrwbd`
    repository_name = "rrwbd",
    tags = ["manual"],
)

alias(
    name = "wasm_bindgen_cli",
    actual = "@wasm_bindgen//:wasm_bindgen_cli",
    tags = ["manual"],
    visibility = ["//visibility:public"],
)

alias(
    name = "wasm_bindgen",
    actual = "//3rdparty/crates:wasm-bindgen",
    visibility = ["//visibility:public"],
)

bzl_library(
    name = "bzl_lib",
    srcs = [
        "//3rdparty/crates:crates.bzl",
        "//3rdparty/crates:defs.bzl",
    ],
    visibility = ["//:__pkg__"],
)

rust_bindgen_toolchain(
    name = "wasm_bindgen_toolchain_impl",
    bindgen = "@wasm_bindgen//:wasm_bindgen_cli",
)

toolchain(
    name = "wasm_bindgen_toolchain",
    toolchain = "wasm_bindgen_toolchain_impl",
    toolchain_type = "@rules_rust_wasm_bindgen//:toolchain_type",
)

load("@rules_rust//rust:defs.bzl", "rust_binary", "rust_shared_library", "rust_library")
load("@rules_pkg//:pkg.bzl", "pkg_tar")
load("@rules_rust_wasm_bindgen//rules_js:defs.bzl", "js_rust_wasm_bindgen", )

platform(
    name = "wasm",
    constraint_values = [
        "@platforms//cpu:wasm32",
        "@platforms//os:none",
    ],
)

rust_binary(
    name = "jaydanhoward_so",
    edition = "2021",
    srcs = glob([
        "src/**/*.rs",
    ]),
    tags = ["manual"],
    crate_features = ["hydrate"],
    rustc_env = {
        "SERVER_FN_OVERRIDE_KEY": "bazel",
    },
    platform = ":wasm",
    visibility = ["//visibility:public"],
    deps = [
        "@wasm_crates//:anyhow",
        "@wasm_crates//:cfg-if",
        "@wasm_crates//:console_error_panic_hook",
        "@wasm_crates//:leptos",
        "@wasm_crates//:leptos_meta",
        "@wasm_crates//:leptos_router",
        "@wasm_crates//:rand",
        "@wasm_crates//:serde",
        "@wasm_crates//:wasm-bindgen",
        "@wasm_crates//:web-sys",
    ]
)

server_deps = [
    "@server_crates//:actix-files",
    "@server_crates//:actix-multipart",
    "@server_crates//:actix-web",
    "@server_crates//:anyhow",
    "@server_crates//:base64",
    "@server_crates//:cfg-if",
    "@server_crates//:config",
    "@server_crates//:console_error_panic_hook",
    "@server_crates//:futures",
    "@server_crates//:futures-util",
    "@server_crates//:leptos",
    "@server_crates//:leptos_actix",
    "@server_crates//:leptos_meta",
    "@server_crates//:leptos_router",
    "@server_crates//:rand",
    "@server_crates//:reqwest",
    "@server_crates//:serde",
    "@server_crates//:serde-aux",
    "@server_crates//:thiserror",
    "@server_crates//:tokio",
    "@server_crates//:tracing",
    "@server_crates//:tracing-bunyan-formatter",
    "@server_crates//:tracing-log",
    "@server_crates//:tracing-subscriber",
    "@rules_rust//tools/runfiles",
]

rust_library(
    name = "jaydanhoward",
    visibility = ["//visibility:public"],
    srcs = glob([
        "src/**/*.rs",
    ]),
    crate_features = ["ssr"],
    edition = "2021",
    deps = server_deps
)

rust_binary(
    name = "jaydanhoward_bin",
    srcs = glob([
        "src/**/*.rs",
    ]),
    crate_features = ["ssr"],
    edition = "2021",
    data = [
        "//pkg:jaydanhoward_wasm",
        "leptos.toml",
        "//assets:static",
        "//assets/fonts:fonts",
        "//assets/fontawesome/css:css",
        "//assets/fontawesome/webfonts:webfonts",
    ],
    rustc_env = {
        "SERVER_FN_OVERRIDE_KEY": "bazel",
    },
    deps = [":jaydanhoward"] + server_deps,
)

exports_files(["tailwind.config.js"])

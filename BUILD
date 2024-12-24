load("@crates//:defs.bzl", "aliases")
load("@rules_rust//rust:defs.bzl", "rust_binary", "rust_shared_library", "rust_library")
load("@rules_pkg//:pkg.bzl", "pkg_tar")
load("@rules_rust_wasm_bindgen//rules_js:defs.bzl", "js_rust_wasm_bindgen", )
load("@rules_rust_bindgen//:defs.bzl", "rust_bindgen_toolchain")

rust_bindgen_toolchain(
    name = "wasm_bindgen_toolchain",
    bindgen = "@wasm_bindgen//:wasm-bindgen",
)

deps = [
        "@crates//:actix-files",
        "@crates//:actix-multipart",
        "@crates//:actix-web",
        "@crates//:anyhow",
        "@crates//:base64",
        "@crates//:cfg-if",
        "@crates//:config",
        "@crates//:console_error_panic_hook",
        "@crates//:futures-util",
        "@crates//:leptos",
        "@crates//:leptos_actix",
        "@crates//:leptos_meta",
        "@crates//:leptos_router",
        "@crates//:pulldown-cmark",
        "@crates//:rand",
        "@crates//:reqwest",
        "@crates//:serde",
        "@crates//:serde-aux",
        "@crates//:thiserror",
        "@crates//:tokio",
        "@crates//:tracing",
        "@crates//:tracing-bunyan-formatter",
        "@crates//:tracing-log",
        "@crates//:tracing-subscriber",
        "@rules_rust//tools/runfiles",
    ]

rust_binary(
    name = "jaydanhoward_wasm",
    visibility = ["//visibility:public"],
    srcs = glob([
        "src/**/*.rs",
    ]),
    crate_features = ["hydrate"],
    rustc_env = {
        "SERVER_FN_OVERRIDE_KEY": "bazel",
    },
    deps = [
        "@wasm_crate//:anyhow",
        "@wasm_crate//:rand",
        "@wasm_crate//:console_error_panic_hook",
        "@wasm_crate//:cfg-if",
        "@wasm_crate//:wasm-bindgen",
        "@wasm_crate//:web-sys",
        "@wasm_crate//:getrandom",
        "@wasm_crate//:leptos",
        "@wasm_crate//:leptos_router",
        "@wasm_crate//:leptos_meta",
    ],
)


js_rust_wasm_bindgen(
    name = "jaydanhoward_wasm_bindgen",
    target = "web",
    wasm_file = ":jaydanhoward_wasm",
)

rust_library(
    name = "jaydanhoward",
    srcs = glob([
        "src/**/*.rs",
    ]),
    crate_features = ["ssr"],
    aliases = aliases({
        "serde-aux": "serde_aux"
    }),
    deps = deps 
)

rust_binary(
    name = "jaydanhoward_bin",
    srcs = glob([
        "src/**/*.rs",
    ]),
    crate_features = ["ssr"],
    data = [
        "leptos.toml",
        "//assets:static",
        "//assets/fonts:fonts",
        "//assets/fontawesome/css:css",
        "//assets/fontawesome/webfonts:webfonts",
    ],
    rustc_env = {
        "SERVER_FN_OVERRIDE_KEY": "bazel",
    },
    deps = deps + [":jaydanhoward"],
    aliases = aliases({
        "serde-aux": "serde_aux"
    }),

)

exports_files(["tailwind.config.js"])

load("@crates//:defs.bzl", "aliases", "all_crate_deps")
load("@rules_rust//rust:defs.bzl", "rust_binary", "rust_shared_library", "rust_library")

rust_library(
    name = "jaydanhoward",
    srcs = glob([
        "src/**/*.rs",
    ]),
    crate_features = ["ssr"],
    aliases = aliases({
        "serde-aux": "serde_aux"
    }),
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
    ]
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
    ],
    rustc_env = {
        "SERVER_FN_OVERRIDE_KEY": "bazel",
    },
    deps = [
        "jaydanhoward",
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
    ]
)

rust_shared_library(
    name = "jaydanhoward.wasm",
    srcs = glob([
        "src/**/*.rs",
    ]),
    crate_features = ["hydrate"],
    crate_name = "jaydanhoward",
    rustc_env = {
        "SERVER_FN_OVERRIDE_KEY": "bazel",
    },
    tags = ["manual"],
    visibility = ["//visibility:public"],
    deps = [
        "@crates//:actix-files",
        "@crates//:actix-web",
        "@crates//:anyhow",
        "@crates//:config",
        "@crates//:cfg-if",
        "@crates//:leptos",
        "@crates//:leptos_actix",
        "@crates//:leptos_meta",
        "@crates//:leptos_router",
        "@crates//:serde",
        "@crates//:tokio",
        "@crates//:tokio-stream",
        "@crates//:tracing",
        "@crates//:tracing-actix-web",
        "@crates//:tracing-flame",
        "@crates//:tracing-subscriber",
        "@crates//:wasm_bindgen",
        "@rules_rust//tools/runfiles",
    ],
)

exports_files(["tailwind.config.js"])

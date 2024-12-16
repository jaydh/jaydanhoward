load("@crates//:defs.bzl", "aliases", "all_crate_deps")
load("@rules_rust//rust:defs.bzl", "rust_binary", "rust_shared_library", "rust_library")

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
    deps = deps + [":jaydanhoward"]
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
    deps = deps,
)

exports_files(["tailwind.config.js"])

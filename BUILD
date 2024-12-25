load("@rules_rust//rust:defs.bzl", "rust_binary", "rust_shared_library", "rust_library")
load("@rules_pkg//:pkg.bzl", "pkg_tar")
load("@rules_rust_wasm_bindgen//rules_js:defs.bzl", "js_rust_wasm_bindgen", )
load("@rules_oci//oci:defs.bzl", "oci_image", "oci_load", "oci_push")
load("@rules_rust//rust:defs.bzl", "rust_clippy")

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

wasm_deps = [
    "@wasm_crates//:anyhow",
    "@wasm_crates//:cfg-if",
    "@wasm_crates//:console_log",
    "@wasm_crates//:console_error_panic_hook",
    "@wasm_crates//:leptos",
    "@wasm_crates//:leptos_meta",
    "@wasm_crates//:leptos_router",
    "@wasm_crates//:log",
    "@wasm_crates//:rand",
    "@wasm_crates//:serde",
    "@wasm_crates//:wasm-bindgen",
    "@wasm_crates//:web-sys",
    "@rules_rust//tools/runfiles",
]

rust_shared_library(
    name = "jaydanhoward",
    edition = "2021",
    srcs = glob([
        "src/**/*.rs",
    ]),
    tags = ["manual"],
    crate_features = ["hydrate"],
    rustc_env = {
        "SERVER_FN_OVERRIDE_KEY": "bazel",
    },
    visibility = ["//visibility:public"],
    deps = wasm_deps
)

js_rust_wasm_bindgen(
    name = "jaydanhoward_wasm",
    target = "web",
    wasm_file = ":jaydanhoward",
    visibility = ["//visibility:public"],
)

rust_binary(
    name = "jaydanhoward_bin",
    srcs = glob([
        "src/**/*.rs",
    ]),
    crate_features = ["ssr"],
    edition = "2021",
    data = [
        ":jaydanhoward_wasm",
        "leptos.toml",
        "//assets:static",
        "//assets/fonts:fonts",
        "//assets/fontawesome/css:css",
        "//assets/fontawesome/webfonts:webfonts",
    ],
    rustc_env = {
        "SERVER_FN_OVERRIDE_KEY": "bazel",
    },
    deps = server_deps,
)

pkg_tar(
    name = "jaydanhoward_tar",
    srcs = [":jaydanhoward_bin"],
    package_dir = "/app",
    include_runfiles = True
)

oci_image(
    name = "jaydanhoward_image",
    base = "@distroless_cc",
    entrypoint = ["/app/jaydanhoward_bin"],
    tars = [
        ":jaydanhoward_tar",
    ],
    workdir = "/app/jaydanhoward_bin.runfiles",
)

oci_load(
    name = "jaydanhoward_image_load",
    image = ":jaydanhoward_image",
    repo_tags = ["harbor.home.local/library/jaydanhoward:latest"]
)

oci_push(
    name = "jaydanhoward_image_push",
    image = ":jaydanhoward_image",
    repository = "harbor.home.local/library/jaydanhoward",
    remote_tags = ["latest"]
)

rust_clippy(
    name = "clippy",
    testonly = True,
    deps = [
        ":jaydanhoward_bin",
    ],
)

exports_files(["tailwind.config.js"])

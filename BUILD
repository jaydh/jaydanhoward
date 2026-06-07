load("@rules_rust//rust:defs.bzl", "rust_binary", "rust_shared_library", "rust_library")
load("@rules_pkg//:pkg.bzl", "pkg_tar")
load("@rules_rust_wasm_bindgen//:defs.bzl", "rust_wasm_bindgen")
load("@rules_oci//oci:defs.bzl", "oci_image", "oci_load", "oci_push", "oci_image_index",)
load("@rules_rust//rust:defs.bzl", "rust_clippy")
load("@rules_shell//shell:sh_test.bzl", "sh_test")
load("@bazel_skylib//lib:selects.bzl", "selects")

platform(
    name = "linux_x86_64_platform",
    constraint_values = [
        "@platforms//os:linux",
        "@platforms//cpu:x86_64",
    ],
)

platform(
    name = "linux_arm64_platform",
    constraint_values = [
        "@platforms//os:linux",
        "@platforms//cpu:arm64",
    ],
)

selects.config_setting_group(
    name = "linux_arm64",
    match_all = ["@platforms//os:linux", "@platforms//cpu:arm64"],
)

selects.config_setting_group(
    name = "linux_x86_64",
    match_all = ["@platforms//os:linux", "@platforms//cpu:x86_64"],
)


selects.config_setting_group(
    name = "macos_arm64",
    match_all = ["@platforms//os:macos", "@platforms//cpu:arm64"],
)

server_deps = [
    "@server_crates//:axum",
    "@server_crates//:tower",
    "@server_crates//:tower-http",
    "@server_crates//:anyhow",
    "@server_crates//:base64",
    "@server_crates//:cfg-if",
    "@server_crates//:config",
    "@server_crates//:console_error_panic_hook",
    "@server_crates//:futures",
    "@server_crates//:futures-util",
    "@server_crates//:leptos",
    "@server_crates//:leptos_axum",
    "@server_crates//:leptos_meta",
    "@server_crates//:leptos_router",
    "@server_crates//:rand",
    "@server_crates//:reqwest",
    "@server_crates//:rustls",
    "@server_crates//:serde",
    "@server_crates//:serde-aux",
    "@server_crates//:serde_json",
    "@server_crates//:thiserror",
    "@server_crates//:chrono",
    "@server_crates//:kube",
    "@server_crates//:k8s-openapi",
    "@server_crates//:sqlx",
    "@server_crates//:tokio",
    "@server_crates//:tokio-stream",
    "@server_crates//:tracing",
    "@server_crates//:tracing-bunyan-formatter",
    "@server_crates//:tracing-log",
    "@server_crates//:tracing-subscriber",
    "@server_crates//:sgp4",
    "@server_crates//:rayon",
    "@rules_rust//tools/runfiles",
]

wasm_deps = [
    "@wasm_crates//:anyhow",
    "@wasm_crates//:cfg-if",
    "@wasm_crates//:console_log",
    "@wasm_crates//:console_error_panic_hook",
    "@wasm_crates//:js-sys",
    "@wasm_crates//:leptos",
    "@wasm_crates//:leptos_meta",
    "@wasm_crates//:leptos_router",
    "@wasm_crates//:log",
    "@wasm_crates//:rand",
    "@wasm_crates//:serde",
    "@wasm_crates//:serde_json",
    "@wasm_crates//:serde-wasm-bindgen",
    "@wasm_crates//:sgp4",
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
    rustc_flags = [
        "-C", "opt-level=z",      # Optimize aggressively for size
        "-C", "codegen-units=1",  # Single codegen unit for better optimization
        "-C", "panic=abort",      # Smaller panic handling
    ],
    visibility = ["//visibility:public"],
    deps = wasm_deps
)

rust_wasm_bindgen(
    name = "jaydanhoward_wasm_unoptimized_wbg118",
    target = "web",
    wasm_file = ":jaydanhoward",
    visibility = ["//visibility:public"],
)

# Optimize WASM with wasm-opt for production
genrule(
    name = "jaydanhoward_wasm_optimized_v2",
    srcs = [":jaydanhoward_wasm_unoptimized_wbg118"],
    outs = [
        "jaydanhoward_wasm/jaydanhoward_wasm_bg.wasm",
        "jaydanhoward_wasm/jaydanhoward_wasm.js",
        "jaydanhoward_wasm/jaydanhoward_wasm_bg.wasm.d.ts",
        "jaydanhoward_wasm/jaydanhoward_wasm.d.ts",
    ],
    cmd = """
        set -e
        # Get the directory containing the unoptimized WASM files
        WASM_DIR=$$(dirname $$(echo $(locations :jaydanhoward_wasm_unoptimized_wbg118) | tr ' ' '\\n' | grep '\\.wasm$$' | head -1))

        # Copy JS and TypeScript declaration files as-is
        cp $$WASM_DIR/jaydanhoward_wasm_unoptimized_wbg118.js $(location jaydanhoward_wasm/jaydanhoward_wasm.js)
        chmod u+w $(location jaydanhoward_wasm/jaydanhoward_wasm.js)
        cp $$WASM_DIR/jaydanhoward_wasm_unoptimized_wbg118_bg.wasm.d.ts $(location jaydanhoward_wasm/jaydanhoward_wasm_bg.wasm.d.ts) 2>/dev/null || touch $(location jaydanhoward_wasm/jaydanhoward_wasm_bg.wasm.d.ts)
        cp $$WASM_DIR/jaydanhoward_wasm_unoptimized_wbg118.d.ts $(location jaydanhoward_wasm/jaydanhoward_wasm.d.ts) 2>/dev/null || touch $(location jaydanhoward_wasm/jaydanhoward_wasm.d.ts)

        # Normalize the JS import module key and WASM binary URL to stable names independent
        # of the Bazel target. wasm-bindgen embeds --out-name in both; renaming the target
        # would change these and break JS/WASM pairing if one file is stale.
        # Handles both single and double-quoted strings (wasm-bindgen uses single quotes for
        # the default WASM URL and double quotes for the import object key).
        python3 -c "
import re, sys
js = open(sys.argv[1]).read()
def norm(s, suffix, stable):
    for q in (chr(39), chr(34)):
        s = re.sub(q + r'[^' + q + r']*' + suffix + q, q + stable + q, s)
    return s
js = norm(js, r'_bg\\.js', './jaydanhoward_wasm_bg.js')
js = norm(js, r'_bg\\.wasm', './jaydanhoward_wasm_bg.wasm')
open(sys.argv[1], 'w').write(js)
" $(location jaydanhoward_wasm/jaydanhoward_wasm.js)

        # Optimize WASM with wasm-opt (-Oz = optimize aggressively for size)
        WASM_OPT_BIN=""" + select({
        ":linux_x86_64": "$(location @wasm_opt_linux_x86_64//:binary)",
        ":linux_arm64": "$(location @wasm_opt_linux_arm64//:binary)",
        ":macos_arm64": "$(location @wasm_opt_macos_arm64//:binary)",
        "//conditions:default": "$(location @wasm_opt_linux_x86_64//:binary)",
    }) + """
        $$WASM_OPT_BIN -Oz --enable-bulk-memory --enable-sign-ext --enable-mutable-globals --enable-nontrapping-float-to-int $$WASM_DIR/jaydanhoward_wasm_unoptimized_wbg118_bg.wasm -o $(location jaydanhoward_wasm/jaydanhoward_wasm_bg.wasm)
        chmod u+w $(location jaydanhoward_wasm/jaydanhoward_wasm_bg.wasm)

        # Normalize WASM import section module name to stable path independent of Bazel target name.
        # wasm-bindgen embeds --out-name in the import module path; renaming the target would change
        # this and break JS/WASM pairing if one file is stale during a rolling deployment.
        python3 -c "
import sys
def r_leb(d, p):
    r, s = 0, 0
    while True:
        b = d[p]; p += 1
        r |= (b & 0x7f) << s; s += 7
        if not (b & 0x80): break
    return r, p
def w_leb(n):
    o = []
    while True:
        b = n & 0x7f; n >>= 7
        if n: b |= 0x80
        o.append(b)
        if not n: break
    return bytes(o)
STABLE = b'./jaydanhoward_wasm_bg.js'
with open(sys.argv[1], 'rb') as f: data = bytearray(f.read())
pos = 8
while pos < len(data):
    sec_start = pos
    sid = data[pos]; pos += 1
    slen, pos = r_leb(data, pos)
    sstart = pos
    if sid == 2:
        n, pos = r_leb(data, pos)
        parts = [w_leb(n)]
        for _ in range(n):
            mlen, pos = r_leb(data, pos)
            mod = bytes(data[pos:pos+mlen]); pos += mlen
            flen, pos = r_leb(data, pos)
            fld = bytes(data[pos:pos+flen]); pos += flen
            kind = data[pos]; pos += 1
            tidx, pos = r_leb(data, pos)
            if b'jaydanhoward' in mod: mod = STABLE
            parts += [w_leb(len(mod)), mod, w_leb(len(fld)), fld, bytes([kind]), w_leb(tidx)]
        new_sec = b''.join(parts)
        new_hdr = bytes([2]) + w_leb(len(new_sec))
        data = bytes(data[:sec_start]) + new_hdr + new_sec + bytes(data[sstart+slen:])
        break
    pos = sstart + slen
with open(sys.argv[1], 'wb') as f: f.write(data)
" $(location jaydanhoward_wasm/jaydanhoward_wasm_bg.wasm)
    """,
    tools = select({
        ":linux_x86_64": ["@wasm_opt_linux_x86_64//:binary"],
        ":linux_arm64": ["@wasm_opt_linux_arm64//:binary"],
        ":macos_arm64": ["@wasm_opt_macos_arm64//:binary"],
        "//conditions:default": ["@wasm_opt_linux_x86_64//:binary"],
    }),
    visibility = ["//visibility:public"],
)

# Alias for convenience - use optimized version
filegroup(
    name = "jaydanhoward_wasm",
    srcs = [":jaydanhoward_wasm_optimized_v2"],
    visibility = ["//visibility:public"],
)

rust_binary(
    name = "jaydanhoward_bin",
    srcs = glob([
        "src/**/*.rs",
    ]),
    compile_data = glob(["migrations/*.sql"], allow_empty = True),
    crate_features = ["ssr"],
    crate_name = "jaydanhoward",
    edition = "2021",
    stamp = 1,
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
        "GIT_SHA": "{STABLE_GIT_COMMIT_SHA}",
    },
    deps = server_deps,
    rustc_flags = [
        "-C", "opt-level=3",
        "-C", "codegen-units=1"
    ],
)

# Platform-specific binaries for OCI images
rust_binary(
    name = "jaydanhoward_bin_linux_amd64",
    srcs = glob([
        "src/**/*.rs",
    ]),
    compile_data = glob(["migrations/*.sql"], allow_empty = True),
    crate_features = ["ssr"],
    crate_name = "jaydanhoward",
    edition = "2021",
    stamp = 1,
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
        "GIT_SHA": "{STABLE_GIT_COMMIT_SHA}",
    },
    deps = server_deps,
    rustc_flags = [
        "-C", "opt-level=3",
        "-C", "codegen-units=1"
    ],
)

rust_binary(
    name = "jaydanhoward_bin_linux_arm64",
    srcs = glob([
        "src/**/*.rs",
    ]),
    compile_data = glob(["migrations/*.sql"], allow_empty = True),
    crate_features = ["ssr"],
    crate_name = "jaydanhoward",
    edition = "2021",
    stamp = 1,
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
        "GIT_SHA": "{STABLE_GIT_COMMIT_SHA}",
    },
    deps = server_deps,
    rustc_flags = [
        "-C", "opt-level=3",
        "-C", "codegen-units=1"
    ],
)

pkg_tar(
    name = "jaydanhoward_tar",
    srcs = [":jaydanhoward_bin"],
    package_dir = "/app",
    include_runfiles = True
)

pkg_tar(
    name = "jaydanhoward_tar_amd64",
    srcs = [":jaydanhoward_bin_linux_amd64"],
    package_dir = "/app",
    include_runfiles = True
)

pkg_tar(
    name = "jaydanhoward_tar_arm64",
    srcs = [":jaydanhoward_bin_linux_arm64"],
    package_dir = "/app",
    include_runfiles = True
)

pkg_tar(
    name = "zstd_lib_amd64",
    symlinks = {
        "usr/lib/x86_64-linux-gnu/libzstd.so.1": "libzstd.so.1.5.7",
    },
    files = {
        "@zstd_deb_amd64//:file": "usr/lib/x86_64-linux-gnu/libzstd.so.1.5.7",
    },
)

pkg_tar(
    name = "zstd_lib_arm64",
    symlinks = {
        "usr/lib/aarch64-linux-gnu/libzstd.so.1": "libzstd.so.1.5.7",
    },
    files = {
        "@zstd_deb_arm64//:file": "usr/lib/aarch64-linux-gnu/libzstd.so.1.5.7",
    },
)

oci_image(
    name = "jaydanhoward_image_amd64",
    base = "@distroless_cc_debian13_linux_amd64",
    entrypoint = ["/app/jaydanhoward_bin_linux_amd64"],
    tars = [
        ":jaydanhoward_tar_amd64",
        ":zstd_lib_amd64",
    ],
    workdir = "/app/jaydanhoward_bin_linux_amd64.runfiles",
)

oci_image(
    name = "jaydanhoward_image_arm64",
    base = "@distroless_cc_debian13_linux_arm64_v8",
    entrypoint = ["/app/jaydanhoward_bin_linux_arm64"],
    tars = [
        ":jaydanhoward_tar_arm64",
        ":zstd_lib_arm64",
    ],
    workdir = "/app/jaydanhoward_bin_linux_arm64.runfiles",
)

oci_push(
    name = "jaydanhoward_image_amd64_push",
    image = ":jaydanhoward_image_amd64",
    repository = "service-harbor-tailscale-ingress-1.tail6a7a6d.ts.net/library/jaydanhoward",
    remote_tags = ["latest-amd64"]
)

oci_push(
    name = "jaydanhoward_image_arm64_push",
    image = ":jaydanhoward_image_arm64",
    repository = "service-harbor-tailscale-ingress-1.tail6a7a6d.ts.net/library/jaydanhoward",
    remote_tags = ["latest-arm64"]
)

# Multi-arch image manifest
oci_image_index(
    name = "jaydanhoward_image_index",
    images = [
        ":jaydanhoward_image_arm64",
        ":jaydanhoward_image_amd64",
    ],
)

oci_push(
    name = "jaydanhoward_image_index_push",
    image = ":jaydanhoward_image_index",
    repository = "service-harbor-tailscale-ingress-1.tail6a7a6d.ts.net/library/jaydanhoward",
    remote_tags = ["latest"]
)

# Security audit CronJob image
pkg_tar(
    name = "security_audit_image_tar",
    srcs = [
        "//security-audit:entrypoint_py",
        "@cargo_audit_linux_x86_64//:binary",
    ],
    package_dir = "/app",
    mode = "0755",
)

oci_image(
    name = "security_audit_image",
    base = "@python_slim_linux_amd64",
    entrypoint = ["/usr/local/bin/python3", "/app/entrypoint.py"],
    tars = [":security_audit_image_tar"],
)

oci_push(
    name = "security_audit_image_push",
    image = ":security_audit_image",
    repository = "service-harbor-tailscale-ingress-1.tail6a7a6d.ts.net/library/security-audit",
    remote_tags = ["latest"],
)

# Convenience target to build all OCI images
filegroup(
    name = "all_images",
    srcs = [
        ":jaydanhoward_image_amd64",
        ":jaydanhoward_image_arm64",
        ":jaydanhoward_image_index",
    ],
)


rust_clippy(
    name = "clippy",
    testonly = True,
    deps = [
        ":jaydanhoward_bin",
    ],
)

# Security audit using cargo-audit
sh_test(
    name = "security_audit",
    srcs = ["//scripts:security_audit.sh"],
    data = [
        "Cargo.server.lock",
        "Cargo.wasm.lock",
    ] + select({
        ":linux_x86_64": ["@cargo_audit_linux_x86_64//:binary"],
        ":linux_arm64": ["@cargo_audit_linux_arm64//:binary"],
        ":macos_arm64": ["@cargo_audit_macos_x86_64//:binary"],
        "//conditions:default": ["@cargo_audit_macos_x86_64//:binary"],
    }),
    tags = ["security", "no-sandbox", "external"],
)

exports_files(["tailwind.config.js"])

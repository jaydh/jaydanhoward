load("//bzl:tailwindcss.bzl", "tailwindcss")
load("@bazel_skylib//lib:selects.bzl", "selects")

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

tailwindcss(
    name = "style",
    src = "tailwind.css",
    target = "//:jaydanhoward",
    binary = select({
        ":linux_arm64": "@tailwind_linux_arm64//file",
        ":linux_x86_64": "@tailwind_x86_64//file",
        ":macos_arm64": "@tailwind_mac_arm64//file",
    }),
    visibility = ["//visibility:public"],
)

filegroup(
    name = "static",
    srcs = glob(["*"]) + [":style"],
    visibility = ["//visibility:public"],
)

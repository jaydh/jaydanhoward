load("//bzl:tailwindcss.bzl", "tailwindcss")


tailwindcss(
    name = "style",
    src = "tailwind.css",
    target = "//:jaydanhoward",
    binary = select({
        "//:linux_arm64": "@tailwind_linux_arm64//file",
        "//:linux_x86_64": "@tailwind_linux_x86_64//file",
        "//:macos_arm64": "@tailwind_mac_arm64//file",
    }),
    visibility = ["//visibility:public"],
)

filegroup(
    name = "static",
    srcs = glob(["*"]) + [":style"],
    visibility = ["//visibility:public"],
)

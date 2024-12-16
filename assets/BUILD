load("//bzl:tailwindcss.bzl", "tailwindcss")

tailwindcss(
    name = "style",
    src = "tailwind.css",
    target = "//:jaydanhoward.wasm",
    visibility = ["//visibility:public"],
)

filegroup(
    name = "static",
    srcs = glob(["*"]) + [":style"],
    visibility = ["//visibility:public"],
)

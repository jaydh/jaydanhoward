genrule(
    name = "wasm-bindgen",
    srcs = ["wasm-bindgen"],
    outs = ["wasm-bindgen-bin"],
    cmd = "cp $(SRCS) $@ && chmod +x $@",
    executable = True,
    visibility = ["//visibility:public"],
)

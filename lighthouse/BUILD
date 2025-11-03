load("@rules_pkg//:pkg.bzl", "pkg_tar")
load("@rules_oci//oci:defs.bzl", "oci_image", "oci_push")

# ==============================================================================
# Bazel OCI Build for Lighthouse
# ==============================================================================
#
# This uses a two-stage approach:
# 1. Base image (lighthouse-base) built with Docker (Dockerfile.base)
#    - Contains Chromium, Node.js, Lighthouse, and chrome user
#    - Built once and pushed to Harbor
# 2. Final image built with Bazel OCI (adds entrypoint script)
#
# To build the base image (one-time setup):
#   cd lighthouse
#   docker build -f Dockerfile.base -t harbor.home.local/library/lighthouse-base:latest .
#   docker push harbor.home.local/library/lighthouse-base:latest
#
# Then build with Bazel:
#   bazel build //lighthouse:lighthouse_image

exports_files([
    "entrypoint.sh",
    "Dockerfile",
    "Dockerfile.base",
])

# Package the entrypoint script
pkg_tar(
    name = "lighthouse_scripts",
    srcs = ["entrypoint.sh"],
    mode = "0755",
    package_dir = "/home/chrome",
)

# Lighthouse image using OCI rules
oci_image(
    name = "lighthouse_image",
    base = "@lighthouse_base",
    tars = [":lighthouse_scripts"],
    entrypoint = ["./entrypoint.sh"],
    workdir = "/home/chrome",
    env = {
        "LEPTOS_SITE_HEALTH_ADDR": "https://jaydanhoward.com/health_check",
        "LEPTOS_SITE_API_ADDR": "https://jaydanhoward.com/api/lighthouse",
        "LEPTOS_SITE_TARGET_ADDR": "https://jaydanhoward.com/about",
    },
    # Run as chrome user (already set in base image)
    user = "chrome",
)

oci_push(
    name = "lighthouse_image_push",
    image = ":lighthouse_image",
    repository = "harbor.home.local/library/lighthouse",
    remote_tags = ["latest"],
)

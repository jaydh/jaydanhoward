"""Rules for handling Debian packages"""

def _deb_archive_impl(ctx):
    ctx.download_and_extract(
        url = ctx.attr.urls,
        sha256 = ctx.attr.sha256,
        type = "ar",
    )

    # Extract the data.tar.* file (common in .deb packages)
    ctx.execute(["tar", "-xf", "data.tar.xz"], quiet = False)
    ctx.execute(["rm", "-f", "data.tar.xz", "control.tar.xz", "control.tar.gz", "data.tar.gz"], quiet = False)

    # Create BUILD file
    ctx.file("BUILD.bazel", ctx.attr.build_file_content)

deb_archive = repository_rule(
    implementation = _deb_archive_impl,
    attrs = {
        "urls": attr.string_list(
            mandatory = True,
            doc = "List of URLs to download the .deb file from",
        ),
        "sha256": attr.string(
            mandatory = True,
            doc = "SHA256 checksum of the .deb file",
        ),
        "build_file_content": attr.string(
            mandatory = True,
            doc = "Content of the BUILD file to create in the extracted repository",
        ),
    },
    doc = "Downloads and extracts a Debian (.deb) package",
)

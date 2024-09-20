"""
This module contains utilities to work with code generated by prost-build.
"""

load("@rules_rust//rust:defs.bzl", "rust_binary", "rust_test")

def generated_files_check(name, srcs, deps, data, manifest_dir):
    rust_test(
        name = name,
        srcs = srcs,
        data = data + [
            "@rules_rust//rust/toolchain:current_rustfmt_files",
            "@com_google_protobuf//:protoc",
            "@com_google_protobuf//:well_known_protos",
        ],
        env = {
            "PROTOC": "$(rootpath @com_google_protobuf//:protoc)",
            # TODO: necessary?
            "PROTOC_INCLUDE": "external/com_google_protobuf/src",
            "CARGO_MANIFEST_DIR": manifest_dir,
            "RUSTFMT": "$(rootpath @rules_rust//rust/toolchain:current_rustfmt_files)",
        },
        deps = deps,
    )

def protobuf_generator(name, srcs, manifest_dir, deps = [], data = []):
    binary_name = "_%s_bin" % name
    rust_binary(
        name = binary_name,
        srcs = srcs,
        data = data,
        deps = deps,
    )

    native.sh_binary(
        name = name,
        data = data + [
            ":" + binary_name,
            "@com_google_protobuf//:well_known_protos",
            "@com_google_protobuf//:protoc",
            "@rules_rust//rust/toolchain:current_rustfmt_files",
        ],
        srcs = ["//bazel:prost_generator.sh"],
        env = {
            "PROTOC": "$(location @com_google_protobuf//:protoc)",
            "PROTOC_INCLUDE": "external/com_google_protobuf/src",
            "CARGO_MANIFEST_DIR": manifest_dir,
            "GENERATOR": "$(location :%s)" % binary_name,
            "RUSTFMT": "$(rootpath @rules_rust//rust/toolchain:current_rustfmt_files)",
        },
        tags = ["local", "manual", "pb-generator"],
    )

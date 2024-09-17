"""
This module defines a macro for running tests using the pocket-ic server from both mainnet and HEAD.
"""
load("@bazel_skylib//lib:paths.bzl", "paths")


def test_using_pocket_ic_server(macro, name, extra_mainnet_tags = [], extra_HEAD_tags = [], **kwargs):
    """
    Declares two targets as defined by the given test macro, one which uses the mainnet pocket-ic server and one that uses the pocket-ic server from HEAD.

    The idea behind this macro is that NNS and other canisters need to be tested
    against the mainnet version of the replica since that version would be active
    when the canisters would be released at that point in time. Therefor tests that
    check these canisters need to run with the mainnet version of the pocket-ic server
    to replicate production as much as possible.

    Additionally not letting canister tests depend on the HEAD version of the pocket-ic server means
    less time spend on CI whenever IC components (which the pocket-ic server depends on) are modified.

    However it's still useful to also test the canisters against the HEAD version of the IC.
    Therefore an additional target is declared that runs the test using the HEAD version of the
    pocket-ic server. Most tests set the extra_HEAD_tags to some tag like "nns_tests_nightly" or
    "fi_tests_nightly" to run it on a schedule.

    In a way this macro is the mirror image of the rs/tests/system_tests.bzl:system_test_nns() macro.

    Args:
      macro: the bazel macro to run. For example: rust_test_suite or rust_test_suite_with_extra_srcs.
      name: the base name of the target.
        The name will be suffixed with "-pocket-ic-server-mainnet" and "-pocket-ic-server-HEAD"
        for the mainnet and HEAD variants of the pocket-ic server respectively,
      extra_mainnet_tags: extra tags assigned to the mainnet pocket-ic server variant.
      extra_HEAD_tags: extra tags assigned to the HEAD pocket-ic server variant.
        Defaults to "manual" to not automatically run this variant.
      **kwargs: the arguments of the bazel macro.
    """
    data = kwargs.pop("data", [])
    env = kwargs.pop("env", {})
    tags = kwargs.pop("tags", [])
    macro(
        name = name + "-pocket-ic-server-mainnet",
        data = data + ["//:mainnet-pocket-ic"],
        env = env | {
            "POCKET_IC_BIN": "$(rootpath //:mainnet-pocket-ic)",
        },
        tags = [tag for tag in tags if tag not in extra_mainnet_tags] + extra_mainnet_tags,
        **kwargs
    )
    macro(
        name = name + "-pocket-ic-server-HEAD",
        data = data + ["//rs/pocket_ic_server:pocket-ic-server"],
        env = env | {
            "POCKET_IC_BIN": "$(rootpath //rs/pocket_ic_server:pocket-ic-server)",
        },
        tags = [tag for tag in tags if tag not in extra_HEAD_tags] + extra_HEAD_tags,
        **kwargs
    )

def _pocket_ic_mainnet_transition_impl(_settings, attr):
    return {
        "//rs/bitcoin/kyt:pocketic_variant": "mainnet",
    }

pocket_ic_mainnet_transition = transition(
    implementation = _pocket_ic_mainnet_transition_impl,
    inputs = [],
    outputs = [
        "//rs/bitcoin/kyt:pocketic_variant",
    ],
)

TestAspectInfo = provider(fields = ["args", "env"])

def _test_aspect_impl(target, ctx):
    data = getattr(ctx.rule.attr, "data", [])
    args = getattr(ctx.rule.attr, "args", [])
    env = getattr(ctx.rule.attr, "env", [])
    args = [ctx.expand_location(arg, data) for arg in args]
    env = {k: ctx.expand_location(v, data) for (k, v) in env.items()}
    return [TestAspectInfo(
        args = args,
        env = env,
    )]

_test_aspect = aspect(_test_aspect_impl)

def _pocket_ic_mainnet_test_impl(ctx):
    test_aspect_info = ctx.attr.test[TestAspectInfo]
    (_, extension) = paths.split_extension(ctx.executable.test.path)
    executable = ctx.actions.declare_file(
        ctx.label.name + extension,
    )
    ctx.actions.write(
        output = executable,
        content = """\
#!/usr/bin/env bash
set -euo pipefail
{commands}
""".format(
            commands = "\n".join([
                " \\\n".join([
                    '{}="{}"'.format(k, v)
                    for k, v in test_aspect_info.env.items()
                ] + [
                    ctx.executable.test.short_path,
                ] + test_aspect_info.args),
            ]),
        ),
        is_executable = True,
    )

    runfiles = ctx.runfiles(files = [executable, ctx.executable.test] + ctx.files.data)
    runfiles = runfiles.merge(ctx.attr.test[DefaultInfo].default_runfiles)
    for data_dep in ctx.attr.data:
        runfiles = runfiles.merge(data_dep[DefaultInfo].default_runfiles)

    return [DefaultInfo(
        executable = executable,
        files = depset(direct = [executable]),
        runfiles = runfiles,
    )]

pocket_ic_mainnet_test = rule(
    _pocket_ic_mainnet_test_impl,
    attrs = {
        "_allowlist_function_transition": attr.label(
            default = "@bazel_tools//tools/allowlists/function_transition_allowlist",
        ),
        "data": attr.label_list(allow_files = True),
        "test": attr.label(
            aspects = [_test_aspect],
            cfg = "target",
            executable = True,
        ),
    },
    cfg = pocket_ic_mainnet_transition,
    executable = True,
    test = True,
)

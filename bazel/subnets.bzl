def _subnets_impl(repository_ctx):
    repository_ctx.file("BUILD.bazel", content = "\n", executable = False)
    parsed = json.decode(repository_ctx.read(repository_ctx.attr.path))
    subnet = parsed["subnets"]["tdb26-jop6k-aogll-7ltgs-eruif-6kk7m-qpktf-gdiqx-mxtrf-vb5e6-eqe"]
    repository_ctx.file(
        "defs.bzl",
        content = "\n".join([
            "POCKET_IC_REV = '{rev}'".format(rev = subnet["rev"]),
            "POCKET_IC_SHA256 = '{sha256}'".format(sha256 = subnet["artifacts"]["pocket-ic"]["sha256"]),
        ]),
        executable = False,
    )

_subnets = repository_rule(
    implementation = _subnets_impl,
    attrs = {
        "path": attr.label(mandatory = True),
    },
)

def subnets(name, path):
    _subnets(name = name, path = path)

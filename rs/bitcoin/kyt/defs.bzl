PocketIcInfo = provider(doc = "", fields = ["type"])

# buildifier: disable=print
def _impl(ctx):
    print("We're using " + ctx.attr.pocketic[PocketIc].type + "!")

pocketic = rule(
    implementation = _impl,
    attrs = {
        "pocketic": attr.label(),
    },
)

def _pocketic_impl(ctx):
    return PocketIcInfo(type = ctx.label.name)

pocketic = rule(
    implementation = _pocketic_impl,
)

# example/transitions/transitions.bzl
def _mainnet_pocket_ic_impl(settings, attr):
    _ignore = (settings, attr)
    return [
        {"//example:favorite_flavor": "LATTE"},
        {"//example:favorite_flavor": "MOCHA"},
    ]

mainnet_pocket_ic = transition(
    implementation = _impl,
    inputs = [],
    outputs = ["//example:favorite_flavor"],
)

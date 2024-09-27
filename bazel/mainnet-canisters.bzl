def _canisters_impl(repository_ctx):
    reponames = repository_ctx.attr.reponames
    filenames = repository_ctx.attr.filenames
    cans = json.decode(repository_ctx.read(repository_ctx.attr.path))
    foo = '''

load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_file")

def deps():
    '''
    for canisterkey in ["registry", "governance"]:
        canisterinfo = cans.get(canisterkey)
        git_commit_id = canisterinfo.get("rev")
        filename = filenames.get(canisterkey)
        sha256 = canisterinfo.get("sha256")
        reponame = reponames.get(canisterkey)
        foo += '''

    http_file(
        name = "{reponame}",
        downloaded_file_path = "{filename}",
        sha256 = "{sha256}",
        url = "https://download.dfinity.systems/ic/{git_commit_id}/canisters/{filename}",
)

        '''.format(git_commit_id = git_commit_id, filename = filename, sha256 = sha256, reponame = reponame)

    repository_ctx.file("BUILD.bazel", content = "\n", executable = False)
    #out = ""
    #for canister_name in cans:
    #    out += "foo = 'bar'"
    #    print(foo)

    repository_ctx.file(
        "defs.bzl",
        content = foo,
        executable = False,
    )

_canisters = repository_rule(
    implementation = _canisters_impl,
    attrs = {
        "path": attr.label(mandatory = True),
        "reponames": attr.string_dict(mandatory = True),
        "filenames": attr.string_dict(mandatory = True),
    },
)

def canisters(name, path, reponames, filenames):
    _canisters(name = name, path = path, reponames = reponames, filenames = filenames)

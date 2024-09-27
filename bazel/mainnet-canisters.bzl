def _canisters_impl(repository_ctx):
    reponames = dict(repository_ctx.attr.reponames)
    filenames = dict(repository_ctx.attr.filenames)
    cans = json.decode(repository_ctx.read(repository_ctx.attr.path))
    canister_keys = cans.keys()

    content = '''

load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_file")

def deps():
    '''

    for canisterkey in canister_keys:
        canisterinfo = cans.pop(canisterkey, None)

        git_commit_id = canisterinfo.get("rev", None)
        if git_commit_id == None:
            fail("bar")

        sha256 = canisterinfo.get("sha256", None)
        if sha256 == None:
            fail("bar")

        filename = filenames.pop(canisterkey, None)
        if filename == None:
            fail("FOO")

        reponame = reponames.pop(canisterkey, None)
        if reponame == None:
            fail("FOO")

        content += '''

    http_file(
        name = "{reponame}",
        downloaded_file_path = "{filename}",
        sha256 = "{sha256}",
        url = "https://download.dfinity.systems/ic/{git_commit_id}/canisters/{filename}",
)

        '''.format(git_commit_id = git_commit_id, filename = filename, sha256 = sha256, reponame = reponame)

    if len(cans.keys()) != 0:
        fail("foo" + ", ".join(cans.keys()))

    if len(reponames.keys()) != 0:
        fail("foo" + ", ".join(reponames.keys()))

    if len(filenames.keys()) != 0:
        fail("foo" + ", ".join(filenames.keys()))

    repository_ctx.file("BUILD.bazel", content = "\n", executable = False)
    repository_ctx.file(
        "defs.bzl",
        content = content,
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

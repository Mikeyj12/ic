"""
This module defines Bazel targets for the mainnet versions of the core NNS, SNS, and ck canisters.
"""

load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_file")

# WASM metadata is a 2-tuple of git commit ID and WASM hash.
CANISTER_NAME_TO_WASM_METADATA = {
    "governance": ("87343a880050ca72b1361138535211f5770dd52e", "8665830c50c9a0dddd996008e537d060a380ac7b6c22237679bd0cecc4ee1044"),
    "ledger": ("b0ade55f7e8999e2842fe3f49df163ba224b71a2", "d0ec2cdeee48e2dbee07c59dfdc3928413de86930242fef0704ab7c1be6c7664"),
    "archive": ("b43280208c32633a29657a1051660324e88a373d", "db0f094005a0e84e243f8f300236be879dcefa412c2fd36d675390caa689d88d"),
    "index": ("b43280208c32633a29657a1051660324e88a373d", "62bbbada301838ad0f6e371415be990ce70e36c6f11267d4ba9fac8ff09aa32d"),
    "root": ("a0207146be211cdff83321c99e9e70baa62733c7", "c280a25dc565f8a42429cb5b969906c4c5a789381e98f6e11c247c91c4dfaac5"),
    "registry": ("87343a880050ca72b1361138535211f5770dd52e", "57c72469f01fd6ea8b5c5a962a1fed9b4ad550bbebdae38c29d5ad330c25c724"),
    "lifeline": ("a0207146be211cdff83321c99e9e70baa62733c7", "76978515223287ece643bc7ca087eb310412b737e2382a73b8ae55fcb458da5b"),
    "genesis-token": ("cf237434877b39d0a94fb5ef84b13ea576a225ac", "31d91cbdfa6e1aae4cc4fee4f611e25f33922bd3d336f4cdc97d511e03b264a7"),
    "cycles-minting": ("77f48ae63af09b6538b1bf33d3accc3bc74d14f8", "3260e795bd3e446a189539ce89d44cb29f7d196b92cdd2e2c75571c062ef1e50"),
    "sns-wasm": ("87343a880050ca72b1361138535211f5770dd52e", "6dd00ebe425ba360be161c880ce3a3b3cda5a3738d6b323a9fd0366debf590ce"),
    "swap": ("87343a880050ca72b1361138535211f5770dd52e", "4ea01d425cd9c6c0a2c4988af03710d7c2377527eb0375067c69a9baff9963f2"),
    "sns_root": ("87343a880050ca72b1361138535211f5770dd52e", "d52d44b6df33de56c5d02ecc8b26fbf7452df8e426d77ecc9f2e3c98a8a70316"),
    "sns_governance": ("87343a880050ca72b1361138535211f5770dd52e", "66156ae6686ac88836c51dc0dc970800bb83c607e83d2448c90336aa0f1ed589"),
    "sns_index": ("3d0b3f10417fc6708e8b5d844a0bac5e86f3e17d", "08ae5042c8e413716d04a08db886b8c6b01bb610b8197cdbe052c59538b924f0"),
    "sns_ledger": ("3d0b3f10417fc6708e8b5d844a0bac5e86f3e17d", "e8942f56f9439b89b13bd8037f357126e24f1e7932cf03018243347505959fd4"),
    "sns_archive": ("3d0b3f10417fc6708e8b5d844a0bac5e86f3e17d", "5c595c2adc7f6d9971298fee2fa666929711e73341192ab70804c783a0eee03f"),
    "ck_btc_index": ("a3831c87440df4821b435050c8a8fcb3745d86f6", "cac207cf438df8c9fba46d4445c097f05fd8228a1eeacfe0536b7e9ddefc5f1c"),
    "ck_btc_ledger": ("a3831c87440df4821b435050c8a8fcb3745d86f6", "4264ce2952c4e9ff802d81a11519d5e3ffdaed4215d5831a6634e59efd72f7d8"),
    "ck_eth_index": ("a3831c87440df4821b435050c8a8fcb3745d86f6", "8104acad6105abb069b2dbc8289692bd63c2d110127f8e91f99db51465962606"),
    "ck_eth_ledger": ("a3831c87440df4821b435050c8a8fcb3745d86f6", "e5c8a297d1c0c6d2ab2253c0280aaefd6e23fe3a8a994fc64706a1f3c3116062"),
}

def canister_url(git_commit_id, filename):
    return "https://download.dfinity.systems/ic/{git_commit_id}/canisters/{filename}".format(
        git_commit_id = git_commit_id,
        filename = filename,
    )

def mainnet_core_nns_canisters():
    """
    Provides Bazel targets for the **core** NNS canisters that are currently deployed to the mainnet.

    This includes: Lifeline, Root, Registry, Governance, ICP Ledger (Index, Archive), CMC, GTC, SNS-W.
    """

    git_commit_id, sha256 = CANISTER_NAME_TO_WASM_METADATA["registry"]
    http_file(
        name = "mainnet_nns_registry_canister",
        downloaded_file_path = "registry-canister.wasm.gz",
        sha256 = sha256,
        url = canister_url(git_commit_id, "registry-canister.wasm.gz"),
    )

    git_commit_id, sha256 = CANISTER_NAME_TO_WASM_METADATA["governance"]
    http_file(
        name = "mainnet_nns_governance_canister",
        downloaded_file_path = "governance-canister.wasm.gz",
        sha256 = sha256,
        url = canister_url(git_commit_id, "governance-canister.wasm.gz"),
    )

    git_commit_id, sha256 = CANISTER_NAME_TO_WASM_METADATA["ledger"]
    http_file(
        name = "mainnet_icp_ledger_canister",
        downloaded_file_path = "ledger-canister_notify-method.wasm.gz",
        sha256 = sha256,
        url = canister_url(git_commit_id, "ledger-canister_notify-method.wasm.gz"),
    )

    git_commit_id, sha256 = CANISTER_NAME_TO_WASM_METADATA["archive"]
    http_file(
        name = "mainnet_icp_ledger-archive-node-canister",
        downloaded_file_path = "ledger-archive-node-canister.wasm.gz",
        sha256 = sha256,
        url = canister_url(git_commit_id, "ledger-archive-node-canister.wasm.gz"),
    )

    git_commit_id, sha256 = CANISTER_NAME_TO_WASM_METADATA["index"]
    http_file(
        name = "mainnet_icp_index_canister",
        downloaded_file_path = "ic-icp-index-canister.wasm.gz",
        sha256 = sha256,
        url = canister_url(git_commit_id, "ic-icp-index-canister.wasm.gz"),
    )

    git_commit_id, sha256 = CANISTER_NAME_TO_WASM_METADATA["root"]
    http_file(
        name = "mainnet_nns_root-canister",
        downloaded_file_path = "root-canister.wasm.gz",
        sha256 = sha256,
        url = canister_url(git_commit_id, "root-canister.wasm.gz"),
    )

    git_commit_id, sha256 = CANISTER_NAME_TO_WASM_METADATA["lifeline"]
    http_file(
        name = "mainnet_nns_lifeline_canister",
        downloaded_file_path = "lifeline-canister.wasm.gz",
        sha256 = sha256,
        url = canister_url(git_commit_id, "lifeline_canister.wasm.gz"),
    )

    git_commit_id, sha256 = CANISTER_NAME_TO_WASM_METADATA["genesis-token"]
    http_file(
        name = "mainnet_nns_genesis-token-canister",
        downloaded_file_path = "genesis-token-canister.wasm.gz",
        sha256 = sha256,
        url = canister_url(git_commit_id, "genesis-token-canister.wasm.gz"),
    )

    git_commit_id, sha256 = CANISTER_NAME_TO_WASM_METADATA["cycles-minting"]
    http_file(
        name = "mainnet_nns_cycles-minting-canister",
        downloaded_file_path = "cycles-minting-canister.wasm.gz",
        sha256 = sha256,
        url = canister_url(git_commit_id, "cycles-minting-canister.wasm.gz"),
    )

    git_commit_id, sha256 = CANISTER_NAME_TO_WASM_METADATA["sns-wasm"]
    http_file(
        name = "mainnet_nns_sns-wasm-canister",
        downloaded_file_path = "sns-wasm-canister.wasm.gz",
        sha256 = sha256,
        url = canister_url(git_commit_id, "sns-wasm-canister.wasm.gz"),
    )

def mainnet_ck_canisters():
    """
    Provides Bazel targets for the latest ckBTC and ckETH canisters published to the mainnet fiduciary subnet.
    """

    git_commit_id, sha256 = CANISTER_NAME_TO_WASM_METADATA["ck_btc_ledger"]
    http_file(
        name = "mainnet_ckbtc_ic-icrc1-ledger",
        downloaded_file_path = "ic-icrc1-ledger.wasm.gz",
        sha256 = sha256,
        url = canister_url(git_commit_id, "ic-icrc1-ledger.wasm.gz"),
    )

    git_commit_id, sha256 = CANISTER_NAME_TO_WASM_METADATA["ck_btc_index"]
    http_file(
        name = "mainnet_ckbtc-index-ng",
        downloaded_file_path = "ic-icrc1-index-ng.wasm.gz",
        sha256 = sha256,
        url = canister_url(git_commit_id, "ic-icrc1-index-ng.wasm.gz"),
    )

    git_commit_id, sha256 = CANISTER_NAME_TO_WASM_METADATA["ck_eth_ledger"]
    http_file(
        name = "mainnet_cketh_ic-icrc1-ledger-u256",
        downloaded_file_path = "ic-icrc1-ledger-u256.wasm.gz",
        sha256 = sha256,
        url = canister_url(git_commit_id, "ic-icrc1-ledger-u256.wasm.gz"),
    )

    git_commit_id, sha256 = CANISTER_NAME_TO_WASM_METADATA["ck_eth_index"]
    http_file(
        name = "mainnet_cketh-index-ng",
        downloaded_file_path = "ic-icrc1-index-ng-u256.wasm.gz",
        sha256 = sha256,
        url = canister_url(git_commit_id, "ic-icrc1-index-ng-u256.wasm.gz"),
    )

def mainnet_sns_canisters():
    """
    Provides Bazel targets for the latest SNS canisters published to the mainnet SNS-W.

    This includes: Root, SNS Governance, Swap, SNS Ledger (Index, Archive).
    """

    git_commit_id, sha256 = CANISTER_NAME_TO_WASM_METADATA["sns_root"]
    http_file(
        name = "mainnet_sns-root-canister",
        downloaded_file_path = "sns-root-canister.wasm.gz",
        sha256 = sha256,
        url = canister_url(git_commit_id, "sns-root-canister.wasm.gz"),
    )

    git_commit_id, sha256 = CANISTER_NAME_TO_WASM_METADATA["sns_governance"]
    http_file(
        name = "mainnet_sns-governance-canister",
        downloaded_file_path = "sns-governance-canister.wasm.gz",
        sha256 = sha256,
        url = canister_url(git_commit_id, "sns-governance-canister.wasm.gz"),
    )

    git_commit_id, sha256 = CANISTER_NAME_TO_WASM_METADATA["swap"]
    http_file(
        name = "mainnet_sns-swap-canister",
        downloaded_file_path = "sns-swap-canister.wasm.gz",
        sha256 = sha256,
        url = canister_url(git_commit_id, "sns-swap-canister.wasm.gz"),
    )

    git_commit_id, sha256 = CANISTER_NAME_TO_WASM_METADATA["sns_ledger"]
    http_file(
        name = "mainnet_ic-icrc1-ledger",
        downloaded_file_path = "ic-icrc1-ledger.wasm.gz",
        sha256 = sha256,
        url = canister_url(git_commit_id, "ic-icrc1-ledger.wasm.gz"),
    )

    git_commit_id, sha256 = CANISTER_NAME_TO_WASM_METADATA["sns_archive"]
    http_file(
        name = "mainnet_ic-icrc1-archive",
        downloaded_file_path = "ic-icrc1-archive.wasm.gz",
        sha256 = sha256,
        url = canister_url(git_commit_id, "ic-icrc1-archive.wasm.gz"),
    )

    git_commit_id, sha256 = CANISTER_NAME_TO_WASM_METADATA["sns_index"]
    http_file(
        name = "mainnet_ic-icrc1-index-ng",
        downloaded_file_path = "ic-icrc1-index-ng.wasm.gz",
        sha256 = sha256,
        url = canister_url(git_commit_id, "ic-icrc1-index-ng.wasm.gz"),
    )

#!/usr/bin/env python3
"""Bump nix version and hashes in nix-artifacts.json."""

import json
import os
import re
import subprocess
import sys
import urllib.request


ARTIFACTS_JSON = os.path.join(
    os.path.dirname(__file__),
    "..",
    "lib",
    "container-test-driver",
    "nix-artifacts.json",
)

INSTALLER_URL = "https://github.com/NixOS/nix-installer/releases/download/{version}/nix-installer-{arch}-linux"
ARCHES = ["x86_64", "aarch64"]


def get_latest_nix_installer_version() -> str:
    """Fetch the latest stable nix-installer release version from GitHub."""
    headers = {"Accept": "application/vnd.github+json"}
    token = os.environ.get("GITHUB_TOKEN")
    if token:
        headers["Authorization"] = f"Bearer {token}"
    request = urllib.request.Request(
        "https://api.github.com/repos/NixOS/nix-installer/releases",
        headers=headers,
    )
    with urllib.request.urlopen(request) as response:
        releases = json.loads(response.read())
    return next(
        release["tag_name"]
        for release in releases
        if not release["prerelease"]
        and re.match(r"^v?\d+\.\d+\.\d+$", release["tag_name"])
    ).lstrip("v")


def prefetch_url(url: str) -> str:
    """Prefetch a URL and return its hash."""
    result = subprocess.run(
        ["nix-prefetch-url", "--type", "sha256", url],
        capture_output=True,
        text=True,
        check=True,
    )
    return result.stdout.strip()


def main() -> None:
    artifacts_path = os.path.normpath(ARTIFACTS_JSON)
    if os.path.exists(artifacts_path):
        with open(artifacts_path) as f:
            data = json.load(f)
        current_version = data["nixVersion"]
    else:
        current_version = None
    latest_version = get_latest_nix_installer_version()

    if current_version == latest_version:
        print(f"Already at latest Nix version {current_version}")
        sys.exit(0)

    print(f"Nix: {current_version} -> {latest_version}")

    hashes: dict[str, str] = {}
    for arch in ARCHES:
        url = INSTALLER_URL.format(version=latest_version, arch=arch)
        print(f"  Prefetching nix-installer {arch}...")
        hashes[f"{arch}-linux"] = prefetch_url(url)

    new_data = {"nixVersion": latest_version, "nix-installer": hashes}

    with open(artifacts_path, "w") as f:
        json.dump(new_data, f, indent=2)
        f.write("\n")
    print(f"Updated {artifacts_path}")


if __name__ == "__main__":
    main()

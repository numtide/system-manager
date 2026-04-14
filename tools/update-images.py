#! /usr/bin/env nix-shell
#! nix-shell -i python3 -p python3 python3Packages.beautifulsoup4 python3Packages.requests nix-prefetch

import json
import re
import subprocess
import sys
from datetime import datetime
from pathlib import Path

import requests
from bs4 import BeautifulSoup

IMAGES_JSON = (
    Path(__file__).resolve().parent.parent
    / "lib"
    / "container-test-driver"
    / "images.json"
)

# distro -> upstream index URL (must end with "/")
UBUNTU_RELEASES = {
    "ubuntu-22_04": "https://cloud-images.ubuntu.com/releases/jammy/",
    "ubuntu-24_04": "https://cloud-images.ubuntu.com/releases/noble/",
}

DEBIAN_RELEASES = {
    "debian-13": "https://cloud.debian.org/images/cloud/trixie/",
}


def nix_hash(url: str) -> str:
    print(f"[+] nix-prefetch-url {url}", file=sys.stderr)
    result = subprocess.run(
        ["nix-prefetch-url", "--type", "sha256", url],
        check=True,
        capture_output=True,
        text=True,
    )
    return result.stdout.strip()


def latest_dated_subdir(index_url: str, pattern: re.Pattern) -> str:
    """Return the lexically newest dated subdirectory under index_url that matches pattern."""
    page = requests.get(index_url, timeout=30)
    page.raise_for_status()
    soup = BeautifulSoup(page.content, "html.parser")
    candidates = []
    for link in soup.find_all("a"):
        href = link.get("href", "")
        m = pattern.match(href)
        if m:
            candidates.append((m.group("date"), href))
    if not candidates:
        raise RuntimeError(f"no dated subdirectories matched at {index_url}")
    candidates.sort(key=lambda kv: datetime.strptime(kv[0][:8], "%Y%m%d"))
    return candidates[-1][1]


def ubuntu_rootfs(release: str, base_url: str) -> dict:
    """Return { system: { url, sha256 } } for the latest release-* build under base_url."""
    pattern = re.compile(r"^release-(?P<date>\d{8})/$")
    latest = latest_dated_subdir(base_url, pattern)
    build_url = f"{base_url}{latest}"
    print(f"[+] {release}: {build_url}", file=sys.stderr)

    page = requests.get(build_url, timeout=30)
    page.raise_for_status()
    soup = BeautifulSoup(page.content, "html.parser")

    # Filenames look like ubuntu-22.04-server-cloudimg-amd64-root.tar.xz
    rootfs_re = re.compile(
        r"^.*-server-cloudimg-(?P<arch>amd64|arm64)-root\.tar\.xz$"
    )
    arch_to_system = {"amd64": "x86_64-linux", "arm64": "aarch64-linux"}

    out = {}
    for link in soup.find_all("a"):
        href = link.get("href", "")
        m = rootfs_re.match(href)
        if not m:
            continue
        system = arch_to_system[m.group("arch")]
        url = f"{build_url}{href}"
        out[system] = {"url": url, "sha256": nix_hash(url)}
    if not out:
        raise RuntimeError(f"no rootfs tarballs found under {build_url}")
    return out


def debian_genericcloud(release: str, base_url: str) -> dict:
    """Return { system: { url, sha256 } } for the latest dated debian build."""
    pattern = re.compile(r"^(?P<date>\d{8}-\d{4})/$")
    latest = latest_dated_subdir(base_url, pattern)
    build_url = f"{base_url}{latest}"
    print(f"[+] {release}: {build_url}", file=sys.stderr)

    page = requests.get(build_url, timeout=30)
    page.raise_for_status()
    soup = BeautifulSoup(page.content, "html.parser")

    # Filenames look like debian-13-genericcloud-amd64-20260413-2447.tar.xz
    tarball_re = re.compile(
        r"^.*-genericcloud-(?P<arch>amd64|arm64)-\d{8}-\d{4}\.tar\.xz$"
    )
    arch_to_system = {"amd64": "x86_64-linux", "arm64": "aarch64-linux"}

    out: dict[str, dict] = {}
    seen_urls: set[str] = set()
    for link in soup.find_all("a"):
        href = link.get("href", "")
        m = tarball_re.match(href)
        if not m:
            continue
        url = f"{build_url}{href}"
        if url in seen_urls:
            continue
        seen_urls.add(url)
        system = arch_to_system[m.group("arch")]
        out[system] = {"url": url, "sha256": nix_hash(url)}
    if not out:
        raise RuntimeError(f"no genericcloud tarballs found under {build_url}")
    return out


def main() -> None:
    images: dict[str, dict] = {}

    for release, url in UBUNTU_RELEASES.items():
        images[release] = ubuntu_rootfs(release, url)

    for release, url in DEBIAN_RELEASES.items():
        images[release] = debian_genericcloud(release, url)

    IMAGES_JSON.write_text(json.dumps(images, indent=2, sort_keys=True) + "\n")
    print(f"[+] wrote {IMAGES_JSON}", file=sys.stderr)


if __name__ == "__main__":
    main()

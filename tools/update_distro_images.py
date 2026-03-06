#!/usr/bin/env python3
"""Update LXC container image URLs and hashes in distros.nix."""

import html.parser
import re
import subprocess
import sys
import urllib.parse
import urllib.request
from pathlib import Path

DISTROS_NIX = Path(__file__).parent.parent / "lib" / "container-test-driver" / "distros.nix"

# Each entry maps a stable URL pattern (regex matching the portion of
# the URL that is constant across image rotations) to its listing URL.
# The pattern must match exactly one URL in distros.nix per entry.
LXC_IMAGES: list[dict] = [
    {
        "name": "debian amd64",
        "listing_url": "https://images.linuxcontainers.org/images/debian/bookworm/amd64/cloud",
        "url_pattern": r"https://images\.linuxcontainers\.org/images/debian/bookworm/amd64/cloud/[^\"]+",
    },
    {
        "name": "debian arm64",
        "listing_url": "https://images.linuxcontainers.org/images/debian/bookworm/arm64/cloud",
        "url_pattern": r"https://images\.linuxcontainers\.org/images/debian/bookworm/arm64/cloud/[^\"]+",
    },
    {
        "name": "fedora amd64",
        "listing_url": "https://images.linuxcontainers.org/images/fedora/42/amd64/cloud",
        # Matches: .../fedora/42/amd64/cloud/<any-timestamp>/rootfs.tar.xz
        "url_pattern": r"https://images\.linuxcontainers\.org/images/fedora/\d+/amd64/cloud/[^\"]+",
    },
    {
        "name": "fedora arm64",
        "listing_url": "https://images.linuxcontainers.org/images/fedora/42/arm64/cloud",
        "url_pattern": r"https://images\.linuxcontainers\.org/images/fedora/\d+/arm64/cloud/[^\"]+",
    },
    {
        "name": "rocky amd64",
        "listing_url": "https://images.linuxcontainers.org/images/rockylinux/9/amd64/cloud",
        "url_pattern": r"https://images\.linuxcontainers\.org/images/rockylinux/\d+/amd64/cloud/[^\"]+",
    },
    {
        "name": "rocky arm64",
        "listing_url": "https://images.linuxcontainers.org/images/rockylinux/9/arm64/cloud",
        "url_pattern": r"https://images\.linuxcontainers\.org/images/rockylinux/\d+/arm64/cloud/[^\"]+",
    },
    {
        "name": "alma amd64",
        "listing_url": "https://images.linuxcontainers.org/images/almalinux/9/amd64/cloud",
        "url_pattern": r"https://images\.linuxcontainers\.org/images/almalinux/\d+/amd64/cloud/[^\"]+",
    },
    {
        "name": "alma arm64",
        "listing_url": "https://images.linuxcontainers.org/images/almalinux/9/arm64/cloud",
        "url_pattern": r"https://images\.linuxcontainers\.org/images/almalinux/\d+/arm64/cloud/[^\"]+",
    },
    {
        "name": "archlinux amd64",
        "listing_url": "https://images.linuxcontainers.org/images/archlinux/current/amd64/cloud",
        "url_pattern": r"https://images\.linuxcontainers\.org/images/archlinux/current/amd64/cloud/[^\"]+",
    },
]


class DirListingParser(html.parser.HTMLParser):
    """Extract directory entry names from an HTML index page."""

    def __init__(self) -> None:
        super().__init__()
        self.entries: list[str] = []

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        if tag == "a":
            for name, value in attrs:
                if name == "href" and value and value.endswith("/") and value != "../":
                    self.entries.append(urllib.parse.unquote(value.rstrip("/")))


def fetch_latest_build(listing_url: str) -> str:
    """Fetch the directory listing and return the latest build timestamp."""
    request = urllib.request.Request(listing_url)
    with urllib.request.urlopen(request) as response:
        body = response.read().decode()
    parser = DirListingParser()
    parser.feed(body)
    if not parser.entries:
        raise RuntimeError(f"No builds found at {listing_url}")
    # Entries are timestamps like "20260409_20:33", lexicographic sort works
    return sorted(parser.entries)[-1]


def prefetch_url(url: str) -> str:
    """Prefetch a URL via nix-prefetch-url and return the base32 hash."""
    result = subprocess.run(
        ["nix-prefetch-url", "--type", "sha256", url],
        capture_output=True,
        text=True,
        check=True,
    )
    return result.stdout.strip()


def main() -> None:
    if not DISTROS_NIX.exists():
        print(f"Error: {DISTROS_NIX} not found", file=sys.stderr)
        sys.exit(1)

    content = DISTROS_NIX.read_text()
    original = content
    any_updated = False

    for image in LXC_IMAGES:
        name = image["name"]
        print(f"Checking {name}...")

        latest_build = fetch_latest_build(image["listing_url"])
        new_url = f'{image["listing_url"]}/{latest_build}/rootfs.tar.xz'

        if new_url in content:
            print(f"  Already up to date ({latest_build})")
            continue

        print(f"  Updating to {latest_build}")
        print(f"  Prefetching {new_url}...")
        new_hash = prefetch_url(new_url)

        # Replace the URL in distros.nix
        url_match = re.search(image["url_pattern"], content)
        if not url_match:
            print(f"  Warning: no URL match for pattern {image['url_pattern']}", file=sys.stderr)
            continue

        old_url = url_match.group(0)
        content = content.replace(old_url, new_url, 1)

        # Replace the sha256 hash on the next line after the URL
        pos = content.find(new_url)
        before = content[:pos]
        after = content[pos:]
        sha_match = re.search(r'(sha256\s*=\s*")[^"]*(")', after)
        if sha_match:
            old_sha = sha_match.group(0)
            new_sha = f'{sha_match.group(1)}{new_hash}{sha_match.group(2)}'
            content = before + after.replace(old_sha, new_sha, 1)

        old_version_match = re.search(r"/fedora/(\d+)/", old_url)
        new_version_match = re.search(r"/fedora/(\d+)/", new_url)
        if "fedora" in name and old_version_match and new_version_match:
            old_ver = old_version_match.group(1)
            new_ver = new_version_match.group(1)
            if old_ver != new_ver:
                content = content.replace(f"fedora-{old_ver}", f"fedora-{new_ver}")

        any_updated = True
        print(f"  Done: {old_url} -> {new_url}")

    if not any_updated:
        print("All LXC images are up to date.")
        sys.exit(0)

    if content != original:
        DISTROS_NIX.write_text(content)
        print(f"Updated {DISTROS_NIX}")
    else:
        print("Warning: no changes written despite updates detected", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()

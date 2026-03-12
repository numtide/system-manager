#!/usr/bin/env bash
set -euo pipefail

VERSION=${1:?Usage: $0 VERSION (e.g., v1.0.0)}

if [[ ! "$VERSION" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "Error: VERSION must be in format vX.Y.Z (e.g., v1.0.0)" >&2
    exit 1
fi

VERSION_NUM="${VERSION#v}"
CHANGELOG="CHANGELOG.md"
CARGO_TOML="Cargo.toml"
TEMP_NOTES=$(mktemp)
TEMP_CHANGELOG=$(mktemp)

trap 'rm -f "$TEMP_NOTES" "$TEMP_CHANGELOG"' EXIT

CURRENT_VERSION=$(grep -E '^version = "[0-9]+\.[0-9]+\.[0-9]+"' "$CARGO_TOML" | head -1 | sed 's/.*"\([^"]*\)".*/\1/')
if [[ -z "$CURRENT_VERSION" ]]; then
    echo "Error: Could not determine current version from $CARGO_TOML" >&2
    exit 1
fi

echo "Current version: $CURRENT_VERSION"
echo "New version: $VERSION_NUM"
echo ""

echo "Extracting release notes from $CHANGELOG..."
awk '/^## \[Unreleased\]/,/^## \[[0-9]/ {
    if (/^## \[Unreleased\]/) next
    if (/^## \[[0-9]/) exit
    if (/^\[Unreleased\]:/) exit
    print
}' "$CHANGELOG" > "$TEMP_NOTES"

if [[ ! -s "$TEMP_NOTES" ]]; then
    echo "Error: No content found in [Unreleased] section of $CHANGELOG" >&2
    exit 1
fi

echo "Release notes:"
cat "$TEMP_NOTES"
echo ""

read -p "Create release $VERSION? [y/N] " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Aborted."
    exit 1
fi

echo "Updating $CARGO_TOML version to $VERSION_NUM..."
sed -i "s/^version = \"$CURRENT_VERSION\"/version = \"$VERSION_NUM\"/" "$CARGO_TOML"

echo "Updating Cargo.lock..."
cargo check --quiet 2>/dev/null || cargo update --quiet

echo "Updating $CHANGELOG..."
TODAY=$(date +%Y-%m-%d)

awk -v version="$VERSION_NUM" -v date="$TODAY" '
/^## \[Unreleased\]/ {
    print "## [Unreleased]"
    print ""
    print "## [" version "] - " date
    next
}
{ print }
' "$CHANGELOG" > "$TEMP_CHANGELOG"

sed -i "s|\[unreleased\]: https://github.com/numtide/system-manager/compare/v[0-9.]*\.\.\.HEAD|[unreleased]: https://github.com/numtide/system-manager/compare/v$VERSION_NUM...HEAD|" "$TEMP_CHANGELOG"

if ! grep -q "^\[$VERSION_NUM\]:" "$TEMP_CHANGELOG"; then
    sed -i "/^\[unreleased\]:/a [$VERSION_NUM]: https://github.com/numtide/system-manager/compare/v$CURRENT_VERSION...v$VERSION_NUM" "$TEMP_CHANGELOG"
fi

mv "$TEMP_CHANGELOG" "$CHANGELOG"

echo "Committing version updates..."
git add "$CARGO_TOML" Cargo.lock "$CHANGELOG"
git commit -m "chore: bump version to $VERSION_NUM"

echo "Creating git tag $VERSION..."
git tag -a "$VERSION" -m "Release $VERSION"
git push origin HEAD "$VERSION"

echo "Creating GitHub draft release..."
gh release create "$VERSION" \
    --draft \
    --title "system-manager $VERSION" \
    --notes-file "$TEMP_NOTES"

echo ""
echo "Release $VERSION created successfully!"
echo ""
echo "Next steps:"
echo "1. Review the draft release on GitHub"
echo "2. Publish the release when ready"

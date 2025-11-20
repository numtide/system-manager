#!/usr/bin/env bash
set -euo pipefail

VERSION=${1:?Usage: $0 VERSION (e.g., v1.0.0)}

if [[ ! "$VERSION" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "Error: VERSION must be in format vX.Y.Z (e.g., v1.0.0)" >&2
    exit 1
fi

CHANGELOG="CHANGELOG.md"
TEMP_NOTES=$(mktemp)

trap 'rm -f "$TEMP_NOTES"' EXIT

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

read -p "Create tag and draft release $VERSION? [y/N] " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Aborted."
    exit 1
fi

echo "Creating git tag $VERSION..."
git tag -a "$VERSION" -m "Release $VERSION"
git push origin "$VERSION"

echo "Creating GitHub draft release..."
gh release create "$VERSION" \
    --draft \
    --title "system-manager $VERSION" \
    --notes-file "$TEMP_NOTES"

echo "✓ Draft release $VERSION created successfully!"
echo ""
echo "Next steps:"
echo "1. Review the draft release on GitHub"
echo "2. Publish the release when ready"
echo "3. Update CHANGELOG.md to move [Unreleased] content to [$VERSION] section"

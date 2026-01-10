"""MkDocs hooks for generating documentation at build time."""

import subprocess
import sys
from pathlib import Path


def on_pre_build(**kwargs):
    """Generate module options documentation before building the site."""
    snippets_dir = Path("./snippets")
    snippets_dir.mkdir(exist_ok=True)

    options_file = snippets_dir / "module-options.md"

    # Build the options markdown using Nix
    print("Generating module options documentation...", file=sys.stderr)
    result = subprocess.run(
        [
            "nix",
            "build",
            "../#docs.x86_64-linux.optionsCommonMark",
            "--no-link",
            "--print-out-paths",
        ],
        capture_output=True,
        text=True,
        cwd=Path(__file__).parent,
    )

    if result.returncode != 0:
        print(f"Warning: Failed to generate options: {result.stderr}", file=sys.stderr)
        # Write a placeholder so the build doesn't fail
        options_file.write_text(
            "*Module options could not be generated. "
            "Run `nix build .#docs.x86_64-linux.optionsCommonMark` to debug.*\n"
        )
        return

    options_path = Path(result.stdout.strip())

    # Copy the generated content
    options_file.write_text(options_path.read_text())
    print(f"Generated {options_file}", file=sys.stderr)

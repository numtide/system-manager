"""Utility functions for the container test driver."""

import re
import time
from collections.abc import Callable
from pathlib import Path

from .errors import Error

CONTAINER_PATH = ":".join([
    "/run/current-system/sw/bin",
    "/nix/var/nix/profiles/default/bin",
    "/usr/sbin",
    "/sbin",
    "/usr/bin",
    "/bin",
    "/usr/local/sbin",
    "/usr/local/bin",
])


def prepare_machine_root(root: Path) -> None:
    """Prepare the machine root directory structure."""
    root.mkdir(parents=True, exist_ok=True)
    root.joinpath("etc").mkdir(parents=True, exist_ok=True)


def pythonize_name(name: str) -> str:
    """Convert a name to a valid Python identifier."""
    return re.sub(r"^[^A-z_]|[^A-z0-9_]", "_", name)


def retry(fn: Callable[[bool], bool], timeout: int = 900) -> None:
    """Call the given function repeatedly, with 1 second intervals,
    until it returns True or a timeout is reached.
    """
    for _ in range(timeout):
        if fn(False):
            return
        time.sleep(1)

    if not fn(True):
        msg = f"action timed out after {timeout} seconds"
        raise Error(msg)

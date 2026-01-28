"""Environment setup for the container test driver."""

import ctypes
import os
import subprocess
from functools import cache
from pathlib import Path
from tempfile import NamedTemporaryFile

# Load the C library
libc = ctypes.CDLL("libc.so.6", use_errno=True)

# Define the mount function
libc.mount.argtypes = [
    ctypes.c_char_p,  # source
    ctypes.c_char_p,  # target
    ctypes.c_char_p,  # filesystemtype
    ctypes.c_ulong,  # mountflags
    ctypes.c_void_p,  # data
]
libc.mount.restype = ctypes.c_int

MS_BIND = 0x1000
MS_REC = 0x4000


def mount(
    source: Path,
    target: Path,
    filesystemtype: str,
    mountflags: int = 0,
    data: str | None = None,
) -> None:
    """A Python wrapper for the mount system call."""
    source_c = ctypes.c_char_p(str(source).encode("utf-8"))
    target_c = ctypes.c_char_p(str(target).encode("utf-8"))
    fstype_c = ctypes.c_char_p(filesystemtype.encode("utf-8"))
    data_c = ctypes.c_char_p(data.encode("utf-8")) if data else None

    result = libc.mount(
        source_c,
        target_c,
        fstype_c,
        ctypes.c_ulong(mountflags),
        data_c,
    )

    if result != 0:
        errno = ctypes.get_errno()
        raise OSError(errno, os.strerror(errno))


@cache
def init_test_environment(*, interactive: bool) -> None:
    """Set up the test environment (/etc/passwd, /etc/group) once.

    Args:
        interactive: If True, skip passwd/group bind mounts. The host already has
                     proper files and overwriting them would break user lookups.
    """
    if interactive:
        return

    passwd_content = """root:x:0:0:Root:/root:/bin/sh
nixbld:x:1000:100:Nix build user:/tmp:/bin/sh
nobody:x:65534:65534:Nobody:/:/bin/sh
"""

    with NamedTemporaryFile(mode="w", delete=False, prefix="test-passwd-") as f:
        f.write(passwd_content)
        passwd_path = f.name

    group_content = """root:x:0:
nixbld:x:100:nixbld
nogroup:x:65534:
"""

    with NamedTemporaryFile(mode="w", delete=False, prefix="test-group-") as f:
        f.write(group_content)
        group_path = f.name

    result = libc.mount(
        ctypes.c_char_p(passwd_path.encode()),
        ctypes.c_char_p(b"/etc/passwd"),
        ctypes.c_char_p(b"none"),
        ctypes.c_ulong(MS_BIND),
        None,
    )
    if result != 0:
        errno = ctypes.get_errno()
        raise OSError(errno, os.strerror(errno), "Failed to mount passwd")

    result = libc.mount(
        ctypes.c_char_p(group_path.encode()),
        ctypes.c_char_p(b"/etc/group"),
        ctypes.c_char_p(b"none"),
        ctypes.c_ulong(MS_BIND),
        None,
    )
    if result != 0:
        errno = ctypes.get_errno()
        raise OSError(errno, os.strerror(errno), "Failed to mount group")


def setup_filesystems(*, interactive: bool) -> None:
    """Set up filesystems for the container.

    This function handles both sandbox (Nix build) and interactive (outside sandbox) modes.
    In sandbox mode, it mounts tmpfs and cgroup2. Outside the sandbox, these are skipped.

    Args:
        interactive: If True, skip sandbox-specific mounts. The host already has
                     /run as tmpfs and cgroup2 mounted.
    """
    if interactive:
        return

    Path("/run").mkdir(parents=True, exist_ok=True)
    subprocess.run(["mount", "-t", "tmpfs", "none", "/run"], check=True)

    subprocess.run(["mount", "-t", "cgroup2", "none", "/sys/fs/cgroup"], check=True)

    Path("/etc/os-release").touch()
    Path("/etc/machine-id").write_text("a5ea3f98dedc0278b6f3cc8c37eeaeac")

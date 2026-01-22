"""Test driver for container-based system-manager testing on Ubuntu."""

import argparse
import ctypes
import os
import re
import shlex
import shutil
import subprocess
import time
import types
import uuid
from collections.abc import Callable
from contextlib import _GeneratorContextManager
from dataclasses import dataclass
from functools import cache, cached_property
from pathlib import Path
from tempfile import NamedTemporaryFile, TemporaryDirectory
from typing import Any

from colorama import Fore, Style

from .logger import AbstractLogger, CompositeLogger, TerminalLogger


@cache
def init_test_environment() -> None:
    """Set up the test environment (network bridge, /etc/passwd) once."""
    subprocess.run(
        ["ip", "link", "add", "br0", "type", "bridge"],
        check=True,
        text=True,
    )
    subprocess.run(["ip", "link", "set", "br0", "up"], check=True, text=True)
    subprocess.run(
        ["ip", "addr", "add", "192.168.1.254/24", "dev", "br0"],
        check=True,
        text=True,
    )

    # Set up minimal passwd file for unprivileged operations
    passwd_content = """root:x:0:0:Root:/root:/bin/sh
nixbld:x:1000:100:Nix build user:/tmp:/bin/sh
nobody:x:65534:65534:Nobody:/:/bin/sh
"""

    with NamedTemporaryFile(mode="w", delete=False, prefix="test-passwd-") as f:
        f.write(passwd_content)
        passwd_path = f.name

    # Set up minimal group file
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


class Error(Exception):
    pass


def prepare_machine_root(root: Path) -> None:
    root.mkdir(parents=True, exist_ok=True)
    root.joinpath("etc").mkdir(parents=True, exist_ok=True)


def pythonize_name(name: str) -> str:
    return re.sub(r"^[^A-z_]|[^A-z0-9_]", "_", name)


def retry(fn: Callable, timeout: int = 900) -> None:
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


class Machine:
    def __init__(
        self,
        name: str,
        rootfs: Path,
        logger: AbstractLogger,
        rootdir: Path,
        out_dir: str,
        profile: Path | None = None,
        host_nix_store: Path | None = None,
        closure_info: Path | None = None,
    ) -> None:
        self.name = name
        self.rootfs = rootfs
        self.profile = profile
        self.host_nix_store = host_nix_store
        self.closure_info = closure_info
        self.out_dir = out_dir
        self.process: subprocess.Popen | None = None
        self.rootdir: Path = rootdir
        self.logger = logger
        self._nix_installed = False

    @cached_property
    def container_pid(self) -> int:
        return self.get_systemd_process()

    def start(self) -> None:
        prepare_machine_root(self.rootdir)
        init_test_environment()

        cmd = [
            "systemd-nspawn",
            "--keep-unit",
            "-M",
            self.name,
            "-D",
            str(self.rootdir),
            "--register=no",
            "--resolv-conf=off",
            "--bind=/proc:/run/host/proc",
            "--bind=/sys:/run/host/sys",
            "--private-network",
            "--network-bridge=br0",
        ]

        if self.host_nix_store:
            cmd.append(f"--bind-ro={self.host_nix_store}:/run/host/nix")

        # Use Ubuntu's systemd as init
        cmd.append("/lib/systemd/systemd")

        env = os.environ.copy()
        env["SYSTEMD_NSPAWN_UNIFIED_HIERARCHY"] = "1"
        self.process = subprocess.Popen(
            cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            env=env,
        )

    def wait_for_boot(self, timeout: int = 120) -> None:
        """Wait for systemd to finish booting inside the container."""
        # First ensure we have the container PID
        print(f"Getting container PID for {self.name}...")
        pid = self.container_pid
        print(f"Container {self.name} has PID {pid}")

        # Poll until systemctl is-system-running returns a boot-complete state
        start_time = time.time()
        last_status = ""
        while time.time() - start_time < timeout:
            result = self.execute("systemctl is-system-running", timeout=10)
            lines = result.stdout.strip().split("\n")
            status = lines[-1].strip() if lines else ""
            if status != last_status:
                print(f"Container {self.name} status: {status}")
                last_status = status
            if status in ("running", "degraded"):
                print(f"Container {self.name} boot complete: {status}")
                return
            if status == "starting":
                time.sleep(1)
                continue
            # Other states like 'initializing' - keep waiting
            time.sleep(1)

        msg = f"Timeout waiting for container {self.name} to boot (last status: {last_status})"
        raise RuntimeError(msg)

    def get_systemd_process(self) -> int:
        if self.process is None:
            msg = "Machine not started"
            raise RuntimeError(msg)

        print(f"Looking for child process of nspawn PID {self.process.pid}")

        # Get the container's init PID from nspawn's child process
        # Wait briefly for the container to start
        for attempt in range(30):
            time.sleep(1)
            try:
                children_path = Path(
                    f"/proc/{self.process.pid}/task/{self.process.pid}/children"
                )
                children = children_path.read_text().split()
                print(f"Attempt {attempt + 1}: found children {children}")
                if len(children) == 1:
                    return int(children[0])
            except FileNotFoundError as e:
                print(f"Attempt {attempt + 1}: {e}")
            except ValueError as e:
                print(f"Attempt {attempt + 1}: parse error {e}")
            # Check if nspawn died
            if self.process.poll() is not None:
                # Try to get any output
                if self.process.stdout:
                    output = self.process.stdout.read()
                    print(f"nspawn output: {output}")
                msg = f"systemd-nspawn exited with code {self.process.returncode}"
                raise RuntimeError(msg)

        msg = f"Timeout waiting for container {self.name} to start"
        raise RuntimeError(msg)

    def get_unit_info(self, unit: str) -> dict[str, str]:
        proc = self.systemctl(f'--no-pager show "{unit}"')
        if proc.returncode != 0:
            msg = (
                f'retrieving systemctl info for unit "{unit}"'
                f" failed with exit code {proc.returncode}"
            )
            raise Error(msg)

        line_pattern = re.compile(r"^([^=]+)=(.*)$")

        def tuple_from_line(line: str) -> tuple[str, str]:
            match = line_pattern.match(line)
            if match is None:
                msg = f"Failed to parse line: {line}"
                raise RuntimeError(msg)
            return match[1], match[2]

        return dict(
            tuple_from_line(line)
            for line in proc.stdout.split("\n")
            if line_pattern.match(line)
        )

    def nsenter_command(self, command: str) -> list[str]:
        nsenter = shutil.which("nsenter")

        if not nsenter:
            msg = "nsenter command not found"
            raise RuntimeError(msg)

        return [
            nsenter,
            "--target",
            str(self.container_pid),
            "--mount",
            "--uts",
            "--ipc",
            "--net",
            "--pid",
            "--cgroup",
            "/bin/bash",
            "-c",
            command,
        ]

    def execute(
        self,
        command: str,
        check_return: bool = True,
        check_output: bool = True,
        timeout: int | None = 900,
    ) -> subprocess.CompletedProcess:
        """Execute a shell command inside the container."""
        path_setup = "export PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:$PATH"
        nix_profile_single = "/nix/var/nix/profiles/default/etc/profile.d/nix.sh"
        nix_profile_daemon = "/nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh"
        source_nix = (
            f"[ -f {nix_profile_daemon} ] && source {nix_profile_daemon} || "
            f"[ -f {nix_profile_single} ] && source {nix_profile_single} || true"
        )
        command = f"set -eo pipefail; {path_setup}; {source_nix}; {command}"

        return subprocess.run(
            self.nsenter_command(command),
            env={},
            timeout=timeout,
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
        )

    def nested(
        self,
        msg: str,
        attrs: dict[str, str] | None = None,
    ) -> _GeneratorContextManager:
        if attrs is None:
            attrs = {}
        my_attrs = {"machine": self.name}
        my_attrs.update(attrs)
        return self.logger.nested(msg, my_attrs)

    def systemctl(self, q: str) -> subprocess.CompletedProcess:
        """Runs systemctl commands."""
        return self.execute(f"systemctl {q}")

    def install_nix_if_needed(self) -> None:
        """Install Nix using nix-installer if not already installed."""
        if self._nix_installed:
            return

        # Check for marker file
        result = self.execute("test -f /.nix-not-installed")
        if result.returncode != 0:
            self._nix_installed = True
            return  # Already installed

        with self.nested("Installing Nix via nix-installer"):
            # Run nix-installer in multi-user mode with daemon
            # Use local tarball for offline installation (no network in sandbox)
            result = self.execute(
                "/usr/local/bin/nix-installer install linux "
                "--no-confirm "
                "--nix-package-url file:///usr/local/share/nix/nix.tar.xz "
                "--extra-conf 'sandbox = false'",
                timeout=300,
            )
            if result.returncode != 0:
                msg = f"Failed to install Nix: {result.stdout}"
                raise Error(msg)

            self.wait_for_unit("nix-daemon.socket", timeout=60)

            self.execute("rm /.nix-not-installed")
            self._nix_installed = True

    def copy_profile_to_container(self) -> None:
        """Copy system-manager profile closure into container's Nix store."""
        if not self.profile:
            return

        with self.nested(f"Copying profile {self.profile} to container"):
            # Host store is bind-mounted at /run/host/nix
            # Copy each store path from closure-info/store-paths
            if self.closure_info:
                closure_info_path = Path(self.closure_info)
                store_paths_file = closure_info_path / "store-paths"
                if store_paths_file.exists():
                    store_paths = store_paths_file.read_text().strip().split("\n")
                    print(f"Copying {len(store_paths)} store paths...")
                    for store_path in store_paths:
                        if not store_path:
                            continue
                        # Extract basename (e.g., "abc123-package" from "/nix/store/abc123-package")
                        basename = Path(store_path).name
                        # Copy from host store to container store (skip if exists)
                        result = self.execute(
                            f"test -e /nix/store/{basename} || cp -a /run/host/nix/{basename} /nix/store/{basename}",
                            timeout=120,
                        )
                        if result.returncode != 0:
                            msg = f"Failed to copy {store_path}: {result.stdout}"
                            raise Error(msg)
                    return

            basename = Path(self.profile).name
            result = self.execute(
                f"cp -a /run/host/nix/{basename} /nix/store/{basename}",
                timeout=300,
            )
            if result.returncode != 0:
                msg = f"Failed to copy profile: {result.stdout}"
                raise Error(msg)

    def wait_until_succeeds(self, command: str, timeout: int = 900) -> str:
        """Repeat a shell command with 1-second intervals until it succeeds."""
        output = ""

        def check_success(_: Any) -> bool:
            nonlocal output
            result = self.execute(command, timeout=timeout)
            output = result.stdout
            return result.returncode == 0

        with self.nested(f"waiting for success: {command}"):
            retry(check_success, timeout)
            return output

    def wait_for_open_port(
        self,
        port: int,
        addr: str = "localhost",
        timeout: int = 900,
    ) -> None:
        """Wait for a port to be open on the given address."""
        command = f"nc -z {shlex.quote(addr)} {port}"
        self.wait_until_succeeds(command, timeout=timeout)

    def wait_for_file(self, filename: str, timeout: int = 30) -> None:
        """Waits until the file exists in the machine's file system."""

        def check_file(_last_try: bool) -> bool:
            result = self.execute(f"test -e {filename}")
            return result.returncode == 0

        with self.nested(f"waiting for file '{filename}'"):
            retry(check_file, timeout)

    def wait_for_unit(self, unit: str, timeout: int = 900) -> None:
        """Wait for a systemd unit to get into 'active' state."""

        def check_active(_: bool) -> bool:
            info = self.get_unit_info(unit)
            state = info.get("ActiveState", "unknown")
            if state == "failed":
                proc = self.systemctl(f"--lines 0 status {unit}")
                journal = self.execute(f"journalctl -u {unit} --no-pager")
                msg = f'unit "{unit}" reached state "{state}":\n{proc.stdout}\n{journal.stdout}'
                raise Error(msg)

            if state == "inactive":
                proc = self.systemctl("list-jobs --full 2>&1")
                if "No jobs" in proc.stdout:
                    info = self.get_unit_info(unit)
                    if info.get("ActiveState") == state:
                        msg = f'unit "{unit}" is inactive and there are no pending jobs'
                        raise Error(msg)

            return state == "active"

        with self.nested(f"waiting for unit '{unit}'"):
            retry(check_active, timeout)

    def succeed(self, command: str, timeout: int | None = None) -> str:
        res = self.execute(command, timeout=timeout)
        if res.returncode != 0:
            msg = f"Failed to run command {command}\n"
            msg += f"Exit code: {res.returncode}\n"
            msg += f"Output: {res.stdout}"
            raise RuntimeError(msg)
        return res.stdout

    def fail(self, command: str, timeout: int | None = None) -> str:
        res = self.execute(command, timeout=timeout)
        if res.returncode == 0:
            msg = f"command `{command}` unexpectedly succeeded\n"
            msg += f"Exit code: {res.returncode}\n"
            msg += f"Output: {res.stdout}"
            raise RuntimeError(msg)
        return res.stdout

    def shutdown(self) -> None:
        """Shut down the machine, waiting for the container to exit."""
        if self.process:
            self.process.terminate()
            self.process.wait()
            self.process = None

    def release(self) -> None:
        self.shutdown()


@dataclass
class UbuntuContainerInfo:
    """Container info for Ubuntu-based containers."""
    name: str
    rootfs: Path
    profile: Path | None = None
    host_nix_store: Path | None = None
    closure_info: Path | None = None

    @property
    def root_dir(self) -> Path:
        return Path(f"/.containers/{self.name}")


def setup_filesystems(container: UbuntuContainerInfo) -> None:
    """Set up filesystems for the container."""
    Path("/run").mkdir(parents=True, exist_ok=True)
    subprocess.run(["mount", "-t", "tmpfs", "none", "/run"], check=True)

    subprocess.run(["mount", "-t", "cgroup2", "none", "/sys/fs/cgroup"], check=True)

    container.root_dir.mkdir(parents=True, exist_ok=True)

    Path("/etc/os-release").touch()
    Path("/etc/machine-id").write_text("a5ea3f98dedc0278b6f3cc8c37eeaeac")


class Driver:
    logger: AbstractLogger

    def __init__(
        self,
        containers: list[UbuntuContainerInfo],
        logger: AbstractLogger,
        testscript: str,
        out_dir: str,
    ) -> None:
        self.containers = containers
        self.testscript = testscript
        self.out_dir = out_dir
        self.logger = logger

        self.tempdir = TemporaryDirectory()
        tempdir_path = Path(self.tempdir.name)

        self.machines = []
        for container in containers:
            setup_filesystems(container)

            # Copy rootfs to container working directory
            # Use --no-preserve=ownership so files become owned by root (current user)
            # instead of preserving Nix store ownership which maps incorrectly in container
            container_rootdir = tempdir_path / container.name
            subprocess.run(
                [
                    "cp",
                    "-r",
                    "--no-preserve=ownership",
                    str(container.rootfs),
                    str(container_rootdir),
                ],
                check=True,
            )

            self.machines.append(
                Machine(
                    name=container.name,
                    rootfs=container.rootfs,
                    rootdir=container_rootdir,
                    out_dir=self.out_dir,
                    logger=self.logger,
                    profile=container.profile,
                    host_nix_store=container.host_nix_store,
                    closure_info=container.closure_info,
                ),
            )

    def start_all(self) -> None:
        """Start all containers and prepare them for testing."""
        init_test_environment()

        for machine in self.machines:
            print(f"Starting {machine.name}")
            machine.start()

            print(f"Waiting for {machine.name} to boot...")
            machine.wait_for_boot()

            machine.install_nix_if_needed()
            machine.copy_profile_to_container()

        for machine in self.machines:
            nspawn_uuid = uuid.uuid4()

            sleep = shutil.which("sleep")
            if sleep is None:
                msg = "sleep command not found"
                raise RuntimeError(msg)
            machine.execute(
                f"systemd-run /bin/sh -c '{sleep} 999999999 && echo {nspawn_uuid}'",
            )

            print(
                f"To attach to container {machine.name} run on the same machine that runs the test:",
            )
            print(
                " ".join(
                    [
                        Style.BRIGHT,
                        Fore.CYAN,
                        "sudo",
                        "nsenter",
                        "--user",
                        "--target",
                        f"$(\\pgrep -f '^/bin/sh.*{nspawn_uuid}')",
                        "--mount",
                        "--uts",
                        "--ipc",
                        "--net",
                        "--pid",
                        "--cgroup",
                        "/bin/sh",
                        "-c",
                        "bash",
                        Style.RESET_ALL,
                    ],
                ),
            )

    def test_symbols(self) -> dict[str, Any]:
        general_symbols = {
            "start_all": self.start_all,
            "machines": self.machines,
            "driver": self,
            "Machine": Machine,
        }
        machine_symbols = {pythonize_name(m.name): m for m in self.machines}
        # If there's exactly one machine, make it available under the name "machine"
        if len(self.machines) == 1:
            (machine_symbols["machine"],) = self.machines
        print(
            "additionally exposed symbols:\n    "
            + ", ".join(m.name for m in self.machines)
            + ",\n    "
            + ", ".join(list(general_symbols.keys())),
        )
        return {**general_symbols, **machine_symbols}

    def test_script(self) -> None:
        """Run the test script"""
        exec(self.testscript, self.test_symbols(), None)

    def run_tests(self) -> None:
        """Run the test script (for non-interactive test runs)"""
        self.test_script()

    def __enter__(self) -> "Driver":
        return self

    def __exit__(
        self,
        exc_type: type[BaseException] | None,
        exc_value: BaseException | None,
        traceback: types.TracebackType | None,
    ) -> None:
        for machine in self.machines:
            machine.release()


def writeable_dir(arg: str) -> Path:
    """Raises an ArgumentTypeError if the given argument isn't a writeable directory."""
    path = Path(arg)
    if not path.is_dir():
        msg = f"{path} is not a directory"
        raise argparse.ArgumentTypeError(msg)
    if not os.access(path, os.W_OK):
        msg = f"{path} is not a writeable directory"
        raise argparse.ArgumentTypeError(msg)
    return path


def main() -> None:
    arg_parser = argparse.ArgumentParser(prog="container-test-driver")
    arg_parser.add_argument(
        "--ubuntu-rootfs",
        type=Path,
        required=True,
        help="Path to the Ubuntu rootfs",
    )
    arg_parser.add_argument(
        "--container-name",
        type=str,
        default="machine",
        help="Name of the container",
    )
    arg_parser.add_argument(
        "--profile",
        type=Path,
        help="system-manager profile to copy into the container",
    )
    arg_parser.add_argument(
        "--host-nix-store",
        type=Path,
        default=Path("/nix/store"),
        help="Path to host's /nix/store (for copying store paths)",
    )
    arg_parser.add_argument(
        "--closure-info",
        type=Path,
        help="Path to closure info (for copying store paths)",
    )
    arg_parser.add_argument(
        "--test-script",
        type=Path,
        required=True,
        help="Path to the test script",
    )
    arg_parser.add_argument(
        "-o",
        "--output-directory",
        default=Path.cwd(),
        help="The directory to write output to",
        type=writeable_dir,
    )
    args = arg_parser.parse_args()

    container = UbuntuContainerInfo(
        name=args.container_name,
        rootfs=args.ubuntu_rootfs,
        profile=args.profile,
        host_nix_store=args.host_nix_store,
        closure_info=args.closure_info,
    )

    logger = CompositeLogger([TerminalLogger()])
    with Driver(
        containers=[container],
        testscript=args.test_script.read_text(),
        out_dir=str(args.output_directory.resolve()),
        logger=logger,
    ) as driver:
        driver.run_tests()

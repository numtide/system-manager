"""Machine class for container-based testing."""

import os
import re
import shlex
import shutil
import subprocess
import threading
import time
from contextlib import _GeneratorContextManager
from functools import cached_property
from pathlib import Path
from typing import Any

import testinfra  # type: ignore[import-untyped]
from colorama import Fore, Style

from .errors import Error
from .logger import AbstractLogger
from .utils import CONTAINER_PATH, prepare_machine_root, retry


class TestInfraBackendNix(testinfra.backend.base.BaseBackend):
    """Testinfra backend that uses the container-test-driver Machine to run commands."""

    NAME = "Nix"

    def __init__(self, host: testinfra.host.Host, *args: Any, **kwargs: Any) -> None:
        super().__init__(host.name, **kwargs)
        self._host = host

    def run(
        self, command: str, *args: str, **kwargs: Any
    ) -> testinfra.backend.base.CommandResult:
        cmd = self.get_command(command, *args)
        result = self._host.execute(cmd)
        return testinfra.backend.base.CommandResult(
            backend=self,
            exit_status=result.returncode,
            command=cmd,
            _stdout=result.stdout,
            _stderr="",
        )


class Machine(testinfra.host.Host):
    """Represents a container machine for testing.

    Inherits from testinfra.host.Host to provide testinfra assertions like:
        machine.user('root').exists
        machine.file('/etc/passwd').is_file
        machine.service('nginx').is_running
    """

    def __init__(
        self,
        name: str,
        rootfs: Path,
        logger: AbstractLogger,
        rootdir: Path,
        out_dir: str,
        bridge_name: str,
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
        self.bridge_name = bridge_name
        self.process: subprocess.Popen[str] | None = None
        self.rootdir: Path = rootdir
        self.logger = logger
        self._nix_installed = False
        self._output_thread: threading.Thread | None = None
        self._stop_streaming = threading.Event()
        testinfra.host.Host.__init__(self, backend=TestInfraBackendNix(self))

    @cached_property
    def container_pid(self) -> int:
        return self.get_systemd_process()

    def _stream_output(self) -> None:
        """Background thread to stream container console output."""
        if self.process is None or self.process.stdout is None:
            return

        prefix = f"{Fore.BLUE}[{self.name}]{Style.RESET_ALL} "
        try:
            for line in self.process.stdout:
                if self._stop_streaming.is_set():
                    break
                # Print each line from container console with prefix
                print(f"{prefix}{line}", end="", flush=True)
        except (ValueError, OSError):
            # Pipe closed or process terminated
            pass

    def start(self) -> None:
        """Start the container."""
        prepare_machine_root(self.rootdir)

        # Build the systemd-nspawn command for Ubuntu
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
            f"--network-bridge={self.bridge_name}",
        ]

        # Bind mount host's /nix/store if provided (for nix copy)
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

        # Start background thread to stream container console output
        self._stop_streaming.clear()
        self._output_thread = threading.Thread(
            target=self._stream_output,
            daemon=True,
            name=f"stream-{self.name}",
        )
        self._output_thread.start()

    def wait_for_boot(self, timeout: int = 120) -> None:
        """Wait for systemd to finish booting inside the container."""
        # First ensure we have the container PID
        print(f"Getting container PID for {self.name}...")
        pid = self.container_pid
        print(f"Container {self.name} has PID {pid}")

        # Poll until systemctl is-system-running returns a boot-complete state
        start_time = time.time()
        last_status = ""
        last_jobs_print = 0.0
        while time.time() - start_time < timeout:
            elapsed = time.time() - start_time
            result = self.execute("systemctl is-system-running", timeout=10)
            # Get the last line of output (skip shell tracing from set -x)
            lines = result.stdout.strip().split("\n")
            status = lines[-1].strip() if lines else ""
            if status != last_status:
                print(f"[{elapsed:.1f}s] Container {self.name} status: {status}")
                last_status = status
            # 'running' or 'degraded' means boot is complete
            if status in ("running", "degraded"):
                print(f"[{elapsed:.1f}s] Container {self.name} boot complete: {status}")
                return
            # Show what's still starting every 5 seconds
            if status == "starting" and elapsed - last_jobs_print >= 5.0:
                jobs_result = self.execute(
                    "systemctl list-jobs --no-pager 2>/dev/null | head -10", timeout=10
                )
                if jobs_result.returncode == 0 and jobs_result.stdout.strip():
                    print(f"[{elapsed:.1f}s] Pending jobs:\n{jobs_result.stdout.strip()}")
                last_jobs_print = elapsed
            time.sleep(1)

        msg = f"Timeout waiting for container {self.name} to boot (last status: {last_status})"
        raise RuntimeError(msg)

    def get_systemd_process(self) -> int:
        """Get the PID of the systemd process inside the container."""
        if self.process is None:
            msg = "Machine not started"
            raise RuntimeError(msg)

        print(f"Looking for child process of nspawn PID {self.process.pid}")
        start_time = time.time()

        # Get the container's init PID from nspawn's child process
        # Wait briefly for the container to start
        for _ in range(30):
            time.sleep(1)
            elapsed = time.time() - start_time
            try:
                children_path = Path(
                    f"/proc/{self.process.pid}/task/{self.process.pid}/children"
                )
                children = children_path.read_text().split()
                if len(children) == 1:
                    print(f"[{elapsed:.1f}s] Found container PID: {children[0]}")
                    return int(children[0])
                print(f"[{elapsed:.1f}s] Waiting for container init (children: {children})")
            except FileNotFoundError as e:
                print(f"[{elapsed:.1f}s] Waiting for container init: {e}")
            except ValueError as e:
                print(f"[{elapsed:.1f}s] Parse error: {e}")
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
        """Get information about a systemd unit."""
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
        """Build an nsenter command to execute in the container namespace."""
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
    ) -> subprocess.CompletedProcess[str]:
        """Execute a shell command inside the container."""
        path_setup = f"export PATH={CONTAINER_PATH}:$PATH"
        # Source Nix profile if installed (try both single-user and multi-user locations)
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
    ) -> _GeneratorContextManager[None]:
        """Create a nested logging context."""
        if attrs is None:
            attrs = {}
        my_attrs = {"machine": self.name}
        my_attrs.update(attrs)
        return self.logger.nested(msg, my_attrs)

    def systemctl(self, q: str) -> subprocess.CompletedProcess[str]:
        """Run a systemctl command."""
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

            # Wait for nix-daemon to be ready
            self.wait_for_unit("nix-daemon.socket", timeout=60)

            # Remove marker
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
                            f"test -e /nix/store/{basename} || "
                            f"cp -a /run/host/nix/{basename} /nix/store/{basename}",
                            timeout=120,
                        )
                        if result.returncode != 0:
                            msg = f"Failed to copy {store_path}: {result.stdout}"
                            raise Error(msg)
                    return

            # Fallback: try direct copy of the profile path
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
        """Wait until the file exists in the machine's file system."""

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
        """Execute a command that must succeed, returning stdout."""
        res = self.execute(command, timeout=timeout)
        if res.returncode != 0:
            msg = f"Failed to run command {command}\n"
            msg += f"Exit code: {res.returncode}\n"
            msg += f"Output: {res.stdout}"
            raise RuntimeError(msg)
        return res.stdout

    def activate(self, profile: str | None = None) -> None:
        """Activate system-manager profile and display the output.

        Args:
            profile: Path to system-manager profile. If None, uses the profile
                     passed to the container.
        """
        if profile is None:
            profile = str(self.profile) if self.profile else None
        if profile is None:
            msg = "No profile specified for activation"
            raise Error(msg)

        activate_cmd = f"{profile}/bin/activate"
        print(f"\n{Fore.CYAN}=== Activating system-manager ==={Style.RESET_ALL}")
        print(f"Profile: {profile}")

        res = self.execute(activate_cmd, timeout=300)

        # Always show the output
        if res.stdout.strip():
            for line in res.stdout.strip().split("\n"):
                print(f"  {line}")

        if res.returncode != 0:
            msg = f"Activation failed with exit code {res.returncode}"
            raise Error(msg)

        print(f"{Fore.GREEN}Activation complete{Style.RESET_ALL}")

    def fail(self, command: str, timeout: int | None = None) -> str:
        """Execute a command that must fail, returning stdout."""
        res = self.execute(command, timeout=timeout)
        if res.returncode == 0:
            msg = f"command `{command}` unexpectedly succeeded\n"
            msg += f"Exit code: {res.returncode}\n"
            msg += f"Output: {res.stdout}"
            raise RuntimeError(msg)
        return res.stdout

    def shutdown(self) -> None:
        """Shut down the machine, waiting for the container to exit."""
        # Stop the output streaming thread
        self._stop_streaming.set()
        if self._output_thread and self._output_thread.is_alive():
            self._output_thread.join(timeout=2)
        self._output_thread = None

        if self.process:
            self.process.terminate()
            self.process.wait()
            self.process = None

    def release(self) -> None:
        """Release the machine resources."""
        self.shutdown()

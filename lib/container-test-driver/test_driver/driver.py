"""Driver class for container-based testing."""

import shutil
import subprocess
import time
import types
import uuid
from dataclasses import dataclass
from pathlib import Path
from tempfile import TemporaryDirectory
from typing import Any

from colorama import Fore, Style

from .environment import init_test_environment, setup_filesystems
from .logger import AbstractLogger
from .machine import Machine
from .utils import CONTAINER_PATH, pythonize_name


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


class Driver:
    """Test driver for managing container-based tests."""

    logger: AbstractLogger

    def __init__(
        self,
        containers: list[UbuntuContainerInfo],
        logger: AbstractLogger,
        testscript: str,
        out_dir: str,
        *,
        interactive: bool = False,
    ) -> None:
        self.containers = containers
        self.testscript = testscript
        self.out_dir = out_dir
        self.logger = logger
        self.interactive = interactive

        # Set up host filesystems (tmpfs, cgroup2) - skipped in interactive mode
        setup_filesystems(interactive=self.interactive)

        # Create a unique bridge name (max 15 chars for Linux interface names)
        self.bridge_name = f"ctd-{uuid.uuid4().hex[:8]}"
        self._create_bridge()

        self.tempdir = TemporaryDirectory()
        tempdir_path = Path(self.tempdir.name)

        self.machines: list[Machine] = []
        for container in containers:
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
                    bridge_name=self.bridge_name,
                ),
            )

    def _create_bridge(self) -> None:
        """Create the network bridge for container networking."""
        subprocess.run(
            ["ip", "link", "add", self.bridge_name, "type", "bridge"],
            check=True,
            text=True,
        )
        subprocess.run(
            ["ip", "link", "set", self.bridge_name, "up"],
            check=True,
            text=True,
        )
        subprocess.run(
            ["ip", "addr", "add", "192.168.1.254/24", "dev", self.bridge_name],
            check=True,
            text=True,
        )

    def _destroy_bridge(self) -> None:
        """Remove the network bridge."""
        subprocess.run(
            ["ip", "link", "delete", self.bridge_name],
            check=False,  # Don't fail if bridge already gone
            text=True,
        )

    def start_all(self) -> None:
        """Start all containers and prepare them for testing."""
        init_test_environment(interactive=self.interactive)
        overall_start = time.time()

        for machine in self.machines:
            print(f"Starting {machine.name}")
            phase_start = time.time()
            machine.start()
            print(f"[{time.time() - phase_start:.1f}s] Container process started")

            # Wait for systemd to be ready
            print(f"Waiting for {machine.name} to boot...")
            phase_start = time.time()
            machine.wait_for_boot()
            print(f"[{time.time() - phase_start:.1f}s] Boot complete")

            # Install Nix if needed and copy profile
            phase_start = time.time()
            machine.install_nix_if_needed()
            print(f"[{time.time() - phase_start:.1f}s] Nix installation complete")

            phase_start = time.time()
            machine.copy_profile_to_container()
            print(f"[{time.time() - phase_start:.1f}s] Profile copy complete")

        print(f"[{time.time() - overall_start:.1f}s] All containers ready")

        # Print debug info
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
                        "--target",
                        f"$(\\pgrep -f '^/bin/sh.*{nspawn_uuid}')",
                        "--mount",
                        "--uts",
                        "--ipc",
                        "--net",
                        "--pid",
                        "--cgroup",
                        "--",
                        "/usr/bin/env",
                        f"PATH={CONTAINER_PATH}",
                        "/bin/bash",
                        Style.RESET_ALL,
                    ],
                ),
            )

    def test_symbols(self) -> dict[str, Any]:
        """Return symbols to expose in the test script namespace."""
        general_symbols: dict[str, Any] = {
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
        """Run the test script."""
        exec(self.testscript, self.test_symbols(), None)

    def run_tests(self) -> None:
        """Run the test script (for non-interactive test runs)."""
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
        self._destroy_bridge()

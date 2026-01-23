"""Test driver for container-based system-manager testing on Ubuntu."""

import argparse
import os
import sys
from pathlib import Path

from colorama import Fore, Style

from .driver import Driver, UbuntuContainerInfo
from .errors import Error
from .logger import CompositeLogger, TerminalLogger
from .machine import Machine

__all__ = [
    "Driver",
    "Error",
    "Machine",
    "UbuntuContainerInfo",
    "generate_driver_symbols",
    "main",
]

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
    """Main entry point for the container test driver CLI."""
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
    arg_parser.add_argument(
        "-i",
        "--interactive",
        action="store_true",
        help="Start container and drop into interactive Python shell for debugging",
    )
    args = arg_parser.parse_args()

    container = UbuntuContainerInfo(
        name=args.container_name,
        rootfs=args.ubuntu_rootfs,
        profile=args.profile,
        host_nix_store=args.host_nix_store,
        closure_info=args.closure_info,
    )

    # Check if running as root (required for systemd-nspawn)
    if os.geteuid() != 0:
        print(f"{Fore.RED}Error: This command must be run as root.{Style.RESET_ALL}")
        print("systemd-nspawn requires root privileges to create containers.")
        sys.exit(1)

    logger = CompositeLogger([TerminalLogger()])
    with Driver(
        containers=[container],
        testscript=args.test_script.read_text(),
        out_dir=str(args.output_directory.resolve()),
        logger=logger,
        interactive=args.interactive,
    ) as driver:
        if args.interactive:
            # Interactive debugging mode with ptpython for tab completion
            import ptpython.ipython

            driver.start_all()
            symbols = driver.test_symbols()

            print(f"\n{Fore.GREEN}=== Interactive Debug Mode ==={Style.RESET_ALL}")
            print("Available objects:")
            print("  machine  - the container (use machine.succeed(), machine.execute(), etc.)")
            print("  driver   - the test driver")
            print("  machines - list of all machines")
            print("\nExample commands:")
            print('  machine.succeed("systemctl status nginx")')
            print('  machine.execute("journalctl -u nginx --no-pager")')
            print("  machine.activate()")
            print('  machine.wait_for_unit("system-manager.target")')
            print("\nUse Tab for completion. Press Ctrl+D to exit.\n")

            history_path = str(args.output_directory / ".container-test-history")
            ptpython.ipython.embed(
                user_ns=symbols,
                history_filename=history_path,
            )
        else:
            driver.run_tests()


def generate_driver_symbols() -> None:
    """Generate a file with symbols available in test scripts.

    This creates a 'driver-symbols' file containing comma-separated symbol names
    that can be used by pyflakes to lint test scripts without false positives
    for undefined names.
    """
    general_symbols = [
        "start_all",
        "machines",
        "driver",
        "Machine",
        "machine",  # Available when there's exactly one machine
    ]
    with open("driver-symbols", "w") as fp:
        fp.write(",".join(general_symbols))

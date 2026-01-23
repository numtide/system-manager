# This file contains type hints that can be prepended to test scripts so they can be
# type checked with mypy.
#
# These type stubs are prepended to user test scripts before running mypy.
# Machine names are appended dynamically by the Nix build process.

from collections.abc import Callable

from test_driver.driver import Driver
from test_driver.machine import Machine  # Also exposed as a symbol in test scripts

# General symbols exposed by Driver.test_symbols()
start_all: Callable[[], None]
machines: list[Machine]
driver: Driver

# Available when there's exactly one machine (the common case)
machine: Machine

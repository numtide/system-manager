---
title: Test your configuration
---

# Test your configuration

system-manager provides a container-based test framework that lets you verify your configuration works correctly before deploying it to real systems.
The test framework runs inside the Nix build sandbox using systemd-nspawn containers, providing a fast and reproducible testing environment.

## Prerequisites

Your Nix installation must have `auto-allocate-uids` enabled for the `uid-range` sandbox feature.
Add this to your Nix configuration (typically `/etc/nix/nix.conf` or `~/.config/nix/nix.conf`):

```ini
auto-allocate-uids = true
```

## Quick start

Add a container test to your project's `flake.nix`:

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    system-manager.url = "github:numtide/system-manager";
  };

  outputs = { self, nixpkgs, system-manager, ... }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
    in
    {
      # Your system-manager configuration
      systemConfigs.default = system-manager.lib.makeSystemConfig {
        modules = [ ./config.nix ];
      };

      # Container test
      checks.${system}.config-test =
        system-manager.lib.containerTest.makeContainerTest {
          hostPkgs = pkgs;
          name = "my-config";
          toplevel = self.systemConfigs.default;
          testScript = ''
            start_all()
            machine.wait_for_unit("multi-user.target")
            machine.succeed("${self.systemConfigs.default}/bin/activate")
            machine.wait_for_unit("system-manager.target")

            # Verify your services
            machine.succeed("systemctl is-active nginx")
            machine.succeed("curl -f http://localhost")
          '';
        };
    };
}
```

Run the test:

```bash
nix flake check -L
# or specifically:
nix build .#checks.x86_64-linux.config-test -L
```

## Test script API

The test script is Python code with access to a `machine` object.
The API is designed to be compatible with NixOS VM tests.

### Available methods

| Method | Description |
|--------|-------------|
| `start_all()` | Start all containers (required at script start) |
| `machine.succeed(cmd)` | Run command, fail test if exit code != 0 |
| `machine.fail(cmd)` | Run command, fail test if exit code == 0 |
| `machine.wait_for_unit(unit)` | Wait for systemd unit to be active |
| `machine.wait_for_file(path)` | Wait for file to exist |
| `machine.wait_for_open_port(port)` | Wait for TCP port to be listening |
| `machine.wait_until_succeeds(cmd)` | Retry command until it succeeds |
| `machine.execute(cmd)` | Run command, return result (does not fail test) |
| `machine.systemctl(args)` | Run systemctl with given arguments |

### Example test script

```python
start_all()

# Wait for Ubuntu systemd to be ready
machine.wait_for_unit("multi-user.target")

# Activate system-manager configuration
machine.succeed("${toplevel}/bin/activate")
machine.wait_for_unit("system-manager.target")

# Verify services are running
machine.succeed("systemctl is-active my-service")

# Verify files are in place
machine.succeed("test -f /etc/my-config.conf")
machine.succeed("grep 'expected_value' /etc/my-config.conf")

# Verify packages are in PATH
machine.succeed("bash --login -c 'which my-tool'")

# Check file permissions
mode = machine.succeed("stat -c %a /etc/my-config.conf").strip()
assert mode == "644", f"expected mode 644, got {mode}"

# Wait for a service to be ready
machine.wait_for_open_port(8080)
machine.succeed("curl -f http://localhost:8080/health")
```

## How it works

The test framework:

1. Builds an Ubuntu 24.04 container image with the nix-installer binary included
2. Starts the container using systemd-nspawn within the Nix build sandbox
3. Installs Nix via nix-installer at container startup (multi-user mode with daemon)
4. Copies your system-manager profile closure into the container via `nix copy`
5. Executes your test script

This provides a realistic testing environment that matches how system-manager is deployed on non-NixOS systems.

## Debugging failed tests

If a test fails, the output will show:

- Which command failed
- Exit code and stdout/stderr
- Container logs and systemd journal entries

For interactive debugging, you can build and run the test driver directly:

```bash
# Build the test with verbose output
nix build .#checks.x86_64-linux.config-test -L

# The driver is also available in passthru
nix build .#checks.x86_64-linux.config-test.driver
./result/bin/run-container-test --help
```

## Comparison with VM tests

The container test driver complements the existing VM tests:

| Aspect | Container tests | VM tests |
|--------|----------------|----------|
| Speed | Fast (no QEMU) | Slower (full VM) |
| Isolation | systemd-nspawn | QEMU VM |
| Requirements | `uid-range` feature | KVM support |
| Kernel | Shared with host | Full kernel |
| Use case | Integration testing | System-level testing |

Container tests are ideal for validating that your configuration activates correctly and services start as expected.
VM tests are better suited for testing kernel-dependent features or scenarios requiring full system isolation.

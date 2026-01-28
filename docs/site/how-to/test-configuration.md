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
            machine.activate()
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
The API is designed to be compatible with NixOS VM tests and includes testinfra integration for expressive assertions.

### Available symbols

The test script has access to these symbols:

| Symbol | Description |
|--------|-------------|
| `start_all()` | Start all containers (required at script start) |
| `subtest(name)` | Context manager to group related assertions with timing |
| `machine` | The container machine (when exactly one container) |
| `machines` | List of all machine objects |
| `driver` | The Driver instance |
| `Machine` | Machine class for type hints |

### Machine methods

| Method | Description |
|--------|-------------|
| `machine.activate()` | Activate system-manager profile and display output |
| `machine.succeed(cmd)` | Run command, fail test if exit code != 0 |
| `machine.fail(cmd)` | Run command, fail test if exit code == 0 |
| `machine.wait_for_unit(unit)` | Wait for systemd unit to be active |
| `machine.wait_for_file(path)` | Wait for file to exist |
| `machine.wait_for_open_port(port)` | Wait for TCP port to be listening |
| `machine.wait_until_succeeds(cmd)` | Retry command until it succeeds |
| `machine.execute(cmd)` | Run command, return result (does not fail test) |
| `machine.systemctl(args)` | Run systemctl with given arguments |

### Testinfra assertions

The `machine` object provides testinfra assertions for declarative checks on services, files, and users.
The testinfra API is well-documented at https://testinfra.readthedocs.io/en/latest/modules.html.
These are more readable and provide better error messages than shell commands.

**Service checks:**

```python
assert machine.service("nginx").is_running
assert machine.service("nginx").is_enabled
```

**File checks:**

```python
# Existence and type
assert machine.file("/etc/foo.conf").exists
assert machine.file("/etc/foo.conf").is_file
assert machine.file("/etc/baz").is_directory
assert machine.file("/etc/link").is_symlink

# Content
assert machine.file("/etc/foo.conf").contains("expected=value")

# Ownership (by uid/gid)
assert machine.file("/etc/foo.conf").uid == 0
assert machine.file("/etc/foo.conf").gid == 0

# Ownership (by name)
assert machine.file("/etc/foo.conf").user == "root"
assert machine.file("/etc/foo.conf").group == "root"
```

**User checks:**

```python
assert machine.user("myuser").exists
```

### Grouping tests with subtest

Use the `subtest` context manager to group related assertions.
Each subtest logs timing information and provides clear output when tests fail.

```python
with subtest("Verify nginx is configured"):
    assert machine.service("nginx").is_running
    assert machine.file("/etc/nginx/nginx.conf").exists

with subtest("Verify application files"):
    assert machine.file("/var/www/index.html").exists
    assert machine.file("/var/www/index.html").contains("Welcome")
```

Output shows each subtest name with timing:

```
Test "Verify nginx is configured" (0.5s)
Test "Verify application files" (0.3s)
```

### Example test script

```python
start_all()

# Wait for Ubuntu systemd to be ready
machine.wait_for_unit("multi-user.target")

# Activate system-manager configuration (displays full activation output)
machine.activate()
machine.wait_for_unit("system-manager.target")

with subtest("Verify services are running"):
    assert machine.service("nginx").is_running
    assert machine.service("my-service").is_enabled

with subtest("Verify configuration files"):
    config = machine.file("/etc/my-config.conf")
    assert config.exists
    assert config.is_file
    assert config.contains("expected_value")
    assert config.user == "root"

with subtest("Verify packages are in PATH"):
    machine.succeed("bash --login -c 'which my-tool'")

with subtest("Verify service is responding"):
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

## Test script validation

Test scripts are automatically validated before execution to catch errors early.

**Type checking with mypy:** Validates that your test script uses the API correctly.
Type hints are automatically prepended so symbols like `machine`, `subtest`, and `start_all` are recognized.

**Linting with pyflakes:** Catches undefined variables and other common mistakes.
The test framework symbols are automatically registered as builtins.

Both checks run during the build before the container starts.
If either fails, you see the error immediately without waiting for container setup.

To disable validation when needed:

```nix
system-manager.lib.containerTest.makeContainerTest {
  hostPkgs = pkgs;
  name = "my-config";
  toplevel = self.systemConfigs.default;
  skipTypeCheck = true;  # Disable mypy
  skipLint = true;       # Disable pyflakes
  testScript = ''
    # ...
  '';
};
```

## Debugging failed tests

If a test fails, the output will show:

- Which command failed
- Exit code and stdout/stderr
- Full system-manager activation output

### Interactive debugging

For interactive debugging, build the test driver and run it with `--interactive`:

```bash
# Build the driver
nix build .#checks.x86_64-linux.config-test.driver

# Run interactively (requires root for systemd-nspawn)
sudo ./result/bin/run-container-test --interactive
```

This starts the container and drops you into a Python REPL where you can run commands:

```python
>>> machine.succeed("systemctl status nginx")
>>> machine.execute("journalctl -u nginx --no-pager")
>>> machine.activate()
>>> machine.wait_for_unit("system-manager.target")
```

Press Ctrl+D to exit and stop the container.

To enter the container shell directly, you can use `nsenter`. The command using
`nsenter` is printed in the test output.

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

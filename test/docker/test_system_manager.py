"""Docker-based tests for system-manager.

These tests validate that system-manager correctly activates and manages
systemd services and configuration files in a containerized environment.
We are using testinfra to interact with the container and verify the expected
state of services and files after system-manager activation.

We validate the configuration examples provided in `examples/example.nix`.
"""


def test_nginx_service(host):
    """Verify nginx service is valid and running after activation."""
    assert host.service("nginx.service").is_valid
    assert host.service("nginx.service").is_running


def test_system_manager_target(host):
    """Verify system-manager.target is active."""
    result = host.run("systemctl is-active system-manager.target")
    assert result.rc == 0


def test_package_installed(host):
    """Verify that the 'fd' package is installed."""
    assert host.run("bash -l -c 'fd --help'").rc == 0


def test_managed_services(host):
    """Verify managed services (service-0 through service-9) are active."""
    for i in range(10):
        service = host.service(f"service-{i}.service")
        assert service.is_valid, f"service-{i} should be valid"


def test_etc_foo_conf(host):
    """Verify /etc/foo.conf exists with expected content."""
    f = host.file("/etc/foo.conf")
    assert f.exists
    assert f.contains("launch_the_rockets = true")


def test_etc_nested_files(host):
    """Verify nested etc files are created correctly."""
    assert host.file("/etc/baz/bar/foo2").exists
    assert host.file("/etc/a/nested/example/foo3").exists
    assert host.file("/etc/a/nested/example2/foo3").exists


def test_tmpfiles_directories(host):
    """Verify systemd-tmpfiles directories are created."""
    assert host.file("/var/tmp/system-manager").is_directory
    sample_dir = host.file("/var/tmp/sample")
    assert sample_dir.is_directory
    assert sample_dir.user == "root"
    assert sample_dir.group == "root"
    assert sample_dir.mode == 0o755


def test_tmpfiles_conf(host):
    """Verify tmpfiles configuration files are deployed."""
    assert host.file("/etc/tmpfiles.d/sample.conf").exists
    assert host.file("/etc/tmpfiles.d/00-system-manager.conf").exists

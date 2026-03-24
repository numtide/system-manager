# Test the --sudo flag functionality
# This test creates a non-root user with sudo privileges and verifies that:
# 1. Running system-manager as non-root without --sudo fails
# 2. Running system-manager with --sudo succeeds
{
  forEachUbuntuImage,
  system-manager,
  system,
  ...
}:

forEachUbuntuImage "sudo" {
  modules = [
    ../../examples/example.nix
  ];
  extraPathsToRegister = [
    system-manager.packages.x86_64-linux.default
  ];
  testScriptFunction =
    { toplevel, hostPkgs, ... }:
    let
      system-manager-cli = system-manager.packages.x86_64-linux.default;
    in
    ''
      # Start all machines in parallel
      start_all()

      vm.wait_for_unit("default.target")

      # Create a test user with sudo privileges (NOPASSWD for testing)
      vm.succeed("useradd -m testuser")
      vm.succeed("echo 'testuser ALL=(ALL) NOPASSWD: ALL' > /etc/sudoers.d/testuser")
      vm.succeed("chmod 440 /etc/sudoers.d/testuser")

      # Verify the user exists and can use sudo
      vm.succeed("su - testuser -c 'sudo whoami' | grep -q root")

      # Test 1: Running as non-root without --sudo should fail
      # The activation requires writing to /etc and /nix/var which needs root
      vm.fail("su - testuser -c '${system-manager-cli}/bin/system-manager activate --store-path ${toplevel} 2>&1'")

      # Test 2: Register and activate with --sudo should succeed
      # First register the profile (creates the symlink)
      vm.succeed("su - testuser -c '${system-manager-cli}/bin/system-manager register --sudo --store-path ${toplevel} 2>&1' | tee /tmp/sudo-register.log")
      vm.succeed("! grep -F 'ERROR' /tmp/sudo-register.log")

      # Then activate
      vm.succeed("su - testuser -c '${system-manager-cli}/bin/system-manager activate --sudo --store-path ${toplevel} 2>&1' | tee /tmp/sudo-activate.log")
      vm.succeed("! grep -F 'ERROR' /tmp/sudo-activate.log")

      # Verify activation worked
      vm.wait_for_unit("system-manager.target")
      vm.succeed("systemctl status service-9.service")
      vm.succeed("test -f /etc/foo.conf")

      # Test 3: Deactivation with --sudo should also work
      # Now that the profile is registered, deactivate can find the engine
      vm.succeed("su - testuser -c '${system-manager-cli}/bin/system-manager deactivate --sudo 2>&1' | tee /tmp/sudo-deactivate.log")
      vm.succeed("! grep -F 'ERROR' /tmp/sudo-deactivate.log")

      # Verify deactivation worked
      vm.fail("systemctl status service-9.service")
      vm.fail("test -f /etc/foo.conf")
    '';
}

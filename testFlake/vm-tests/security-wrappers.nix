# Test security wrappers (SUID/capabilities binaries in /run/wrappers/bin).
# This must run in a VM because setting SUID bits and file capabilities
# requires privileges that the nix build sandbox seccomp filter blocks.
{
  forEachImage,
  system-manager,
  ...
}:

forEachImage "security-wrappers" {
  modules = [
    (
      { pkgs, ... }:
      {
        security.wrappers.ping = {
          owner = "root";
          group = "root";
          capabilities = "cap_net_raw+ep";
          source = "${pkgs.iputils.out}/bin/ping";
        };
      }
    )
  ];
  extraPathsToRegister = _distroName: [ ];
  testScriptFunction =
    { toplevel, hostPkgs, ... }:
    ''
      start_all()

      vm.wait_for_unit("default.target")

      ${system-manager.lib.activateProfileSnippet {
        node = "vm";
        profile = toplevel;
      }}
      vm.wait_for_unit("system-manager.target")

      # Verify the oneshot service completed successfully
      result = vm.succeed("systemctl show suid-sgid-wrappers.service -p Result --value").strip()
      assert result == "success", f"suid-sgid-wrappers.service Result={result}"

      # Tmpfs mount at /run/wrappers exists
      mount_output = vm.succeed("findmnt -n -o FSTYPE /run/wrappers").strip()
      assert mount_output == "tmpfs", f"Expected tmpfs, got: {mount_output}"

      # Wrapper binary exists and is executable
      vm.succeed("test -x /run/wrappers/bin/ping")

      # Wrapper binary has correct ownership
      owner = vm.succeed("stat -c '%U:%G' /run/wrappers/bin/ping").strip()
      assert owner == "root:root", f"Expected root:root ownership, got: {owner}"

      # Capabilities are set on wrapper binary
      caps = vm.succeed("getcap /run/wrappers/bin/ping").strip()
      assert "cap_net_raw" in caps, f"Expected cap_net_raw in capabilities, got: {caps}"

      # /run/wrappers/bin precedes /usr/bin in PATH
      path = vm.succeed("bash --login -c 'echo $PATH'").strip()
      entries = path.split(":")
      wrappers_idx = next(i for i, e in enumerate(entries) if e == "/run/wrappers/bin")
      usr_bin_idx = next(i for i, e in enumerate(entries) if e == "/usr/bin")
      assert wrappers_idx < usr_bin_idx, f"/run/wrappers/bin (index {wrappers_idx}) must come before /usr/bin (index {usr_bin_idx}) in PATH: {path}"

      # Default mount and umount wrappers exist
      vm.succeed("test -x /run/wrappers/bin/mount")
      vm.succeed("test -x /run/wrappers/bin/umount")

      # Shadow binaries must NOT be in system-manager PATH: they are linked
      # against nix store PAM libraries and are incompatible with the host
      # system's PAM configuration on non-NixOS distributions.
      vm.fail("test -e /run/system-manager/sw/bin/passwd")
      vm.fail("test -e /run/system-manager/sw/bin/su")
      vm.fail("test -e /run/system-manager/sw/bin/chsh")

      # Build-time check output exists in toplevel
      vm.succeed("test -d ${toplevel}/checks")

      # Deactivation cleans up wrappers
      ${system-manager.lib.deactivateProfileSnippet {
        node = "vm";
        profile = toplevel;
      }}
      vm.fail("test -e /etc/systemd/system/suid-sgid-wrappers.service")
    '';
}

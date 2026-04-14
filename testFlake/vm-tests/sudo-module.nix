# Test sudo module: sudoers generation, no Nix-built sudo in PATH/wrappers,
# and host sudo works with the generated config.
{
  forEachImage,
  system-manager,
  ...
}:

forEachImage "sudo-module" {
  modules = [
    (
      { ... }:
      {
        security.sudo = {
          enable = true;
          wheelNeedsPassword = false;
          extraRules = [
            {
              groups = [ "sudo" ];
              commands = [
                {
                  command = "ALL";
                  options = [ "NOPASSWD" ];
                }
              ];
            }
          ];
        };

        users.users.testuser = {
          isNormalUser = true;
          uid = 1100;
          group = "testuser";
          extraGroups = [
            "wheel"
            "sudo"
          ];
        };
        users.groups.testuser.gid = 1100;
      }
    )
  ];
  extraPathsToRegister = [ ];
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

      # Verify /etc/sudoers is generated with correct rules
      vm.succeed("test -f /etc/sudoers")
      content = vm.succeed("cat /etc/sudoers")
      assert "%wheel" in content, f"sudoers should contain wheel group, got: {content}"
      assert "%sudo" in content, f"sudoers should contain sudo group, got: {content}"
      assert "NOPASSWD" in content, f"sudoers should contain NOPASSWD, got: {content}"
      assert "@includedir /etc/sudoers.d" in content, f"sudoers should include sudoers.d, got: {content}"

      # Nix-built sudo must not be in system-manager PATH or wrappers
      vm.fail("test -e /run/system-manager/sw/bin/sudo")
      vm.fail("test -e /run/system-manager/sw/bin/sudoedit")
      vm.fail("test -e /run/wrappers/bin/sudo")
      vm.fail("test -e /run/wrappers/bin/sudoedit")

      # Verify testuser can sudo without password
      result = vm.succeed("su - testuser -c 'sudo whoami'").strip()
      assert result == "root", f"sudo whoami should return root, got: {result}"
    '';
}

# Test integration of the home-manager NixOS module with system-manager.
{
  forEachDistro,
  home-manager,
  ...
}:

forEachDistro "home-manager" {
  modules = [
    home-manager.nixosModules.home-manager
    (
      { pkgs, ... }:
      {
        nix.enable = true;
        services.userborn.enable = true;

        users.groups.hmuser.gid = 5000;

        users.users.hmuser = {
          isNormalUser = true;
          uid = 5000;
          group = "hmuser";
          home = "/home/hmuser";
          createHome = true;
          initialPassword = "hmuser";
        };

        home-manager = {
          useGlobalPkgs = true;
          useUserPackages = true;
          backupFileExtension = "bak";
          users.hmuser =
            { pkgs, ... }:
            {
              home.stateVersion = "24.05";
              home.file.".config/system-manager-test/hello.txt".text = "hello from home-manager";
              home.packages = [ pkgs.hello ];
            };
        };
      }
    )
  ];
  testScriptFunction =
    { toplevel, ... }:
    ''
      start_all()

      machine.wait_for_unit("multi-user.target")

      activation_logs = machine.activate()
      for line in activation_logs.split("\n"):
          assert not "ERROR" in line, line
      machine.wait_for_unit("system-manager.target")

      with subtest("home-manager per-user service is present"):
          machine.wait_for_unit("home-manager-hmuser.service")
          status = machine.succeed("systemctl is-active home-manager-hmuser.service").strip()
          assert status == "active", f"Expected home-manager-hmuser.service active, got {status!r}"

      with subtest("home-manager activated the managed file"):
          content = machine.succeed("cat /home/hmuser/.config/system-manager-test/hello.txt").strip()
          assert content == "hello from home-manager", f"Unexpected file content: {content!r}"

      with subtest("home-manager user package is on the user's PATH"):
          resolved = machine.succeed(
              "runuser -u hmuser -- bash -lc 'command -v hello'"
          ).strip()
          assert "/etc/profiles/per-user/hmuser/bin/hello" == resolved, (
              f"Expected hello from per-user profile, got: {resolved!r}"
          )
    '';
}

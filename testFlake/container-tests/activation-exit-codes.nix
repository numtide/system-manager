# Test that activation's exit status says whether the profile was applied:
# 0 when it was (including when it drops a unit that already exited), 1 when
# part of it wasn't (here, a restart systemd refused), and 4 when it was but
# managed units failed afterwards (as NixOS' switch-to-configuration exits).
{
  forEachDistro,
  system-manager,
  system,
  ...
}:

let
  services =
    {
      changing ? "valid",
      failing ? false,
      ranOnce ? false,
    }:
    { pkgs, lib, ... }:
    {
      systemd.services = {
        exit-codes-healthy = {
          wantedBy = [ "system-manager.target" ];
          script = "sleep infinity";
        };

        # A changed unit is restarted on its own, and systemd refuses to
        # restart one it can't load: a simple service with two ExecStart= lines
        # has a bad setting.
        exit-codes-changing = {
          wantedBy = [ "system-manager.target" ];
          serviceConfig.ExecStart =
            if changing == "valid" then
              "${pkgs.coreutils}/bin/sleep infinity"
            else
              [
                "${pkgs.coreutils}/bin/sleep infinity"
                "${pkgs.coreutils}/bin/true"
              ];
        };

        exit-codes-failing = lib.mkIf failing {
          wantedBy = [ "system-manager.target" ];
          serviceConfig.Type = "oneshot";
          script = "exit 1";
        };

        # Inactive once it has run. When a profile drops it, its file is gone,
        # and systemd answers the stop with "not loaded": nothing to stop.
        exit-codes-ran-once = lib.mkIf ranOnce {
          wantedBy = [ "system-manager.target" ];
          serviceConfig.Type = "oneshot";
          script = "true";
        };
      };
    };

  makeToplevel =
    args:
    system-manager.lib.makeSystemConfig {
      modules = [
        {
          nixpkgs.hostPlatform = system;
          system-manager.allowAnyDistro = true;
        }
        (services args)
      ];
    };

  failingToplevel = makeToplevel {
    failing = true;
    ranOnce = true;
  };
  badSettingToplevel = makeToplevel { changing = "bad-setting"; };
in
forEachDistro "activation-exit-codes" {
  modules = [ (services { }) ];
  extraPathsToRegister = [
    failingToplevel
    badSettingToplevel
  ];
  testScriptFunction =
    { toplevel, ... }:
    ''
      start_all()
      machine.wait_for_unit("multi-user.target")

      def activate(profile: str, expected: int) -> str:
          res = machine.execute(f"{profile}/bin/activate 2>&1")
          assert res.returncode == expected, f"Expected exit {expected}, got {res.returncode}:\n{res.stdout}"
          return res.stdout

      with subtest("a complete activation exits 0"):
          activate("${toplevel}", 0)
          machine.wait_for_unit("exit-codes-healthy.service")
          machine.wait_for_unit("exit-codes-changing.service")

      with subtest("a managed unit that fails exits 4 and is named"):
          out = activate("${failingToplevel}", 4)
          assert "units failed after activation: exit-codes-failing.service" in out, out

      with subtest("activating the previous profile again exits 0, dropping a unit that already exited"):
          activate("${toplevel}", 0)

      with subtest("a restart systemd refuses exits 1 and is named"):
          out = activate("${badSettingToplevel}", 1)
          assert "activation incomplete: " in out, out
          assert "exit-codes-changing.service: error reloading" in out, out
          # The rest of the profile was still applied.
          machine.wait_for_unit("exit-codes-healthy.service")

      with subtest("activating the previous profile again exits 0"):
          activate("${toplevel}", 0)
          machine.wait_for_unit("exit-codes-changing.service")
    '';
}

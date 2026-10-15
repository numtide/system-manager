# Test that activation's exit status says whether the profile was applied:
# 0 when it was (including when it drops a unit that already exited), 1 when
# part of it wasn't (here, a restart systemd refused, until an activation
# restarts it, and a mount the profile drops that doesn't unmount, until an
# activation stops it), and 4 when it was but managed units failed (failed
# afterwards, a reload that failed, or a start that failed on a dependency the
# profile doesn't manage; as NixOS' switch-to-configuration exits).
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
      reload ? "succeeds",
      mount ? false,
      dependency ? false,
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

        # A changed unit that can reload is reloaded, and a reload that fails
        # leaves it active, on its old configuration, rather than failed.
        exit-codes-reloading = {
          wantedBy = [ "system-manager.target" ];
          serviceConfig.ExecStart = "${pkgs.coreutils}/bin/sleep infinity";
          serviceConfig.ExecReload = "${pkgs.coreutils}/bin/${
            if reload == "succeeds" then "true" else "false"
          }";
        };
      };

      # New, so started only through system-manager.target. When the unit it
      # requires, which the profile doesn't manage, fails, its start ends with
      # "dependency": inactive, not failed.
      systemd.services.exit-codes-dependent = lib.mkIf dependency {
        wantedBy = [ "system-manager.target" ];
        requires = [ "exit-codes-unmanaged.service" ];
        after = [ "exit-codes-unmanaged.service" ];
        script = "sleep infinity";
      };

      # systemd accepts the stop of a mount, and the job fails when it can't
      # unmount: the mount stays.
      systemd.mounts = lib.optional mount {
        what = "tmpfs";
        where = "/mnt/exitcodes";
        type = "tmpfs";
        wantedBy = [ "system-manager.target" ];
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
  failingReloadToplevel = makeToplevel { reload = "fails"; };
  mountToplevel = makeToplevel { mount = true; };
  dependencyToplevel = makeToplevel { dependency = true; };
in
forEachDistro "activation-exit-codes" {
  modules = [ (services { }) ];
  extraPathsToRegister = [
    failingToplevel
    badSettingToplevel
    failingReloadToplevel
    mountToplevel
    dependencyToplevel
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

      with subtest("activating that profile again retries the restart"):
          out = activate("${badSettingToplevel}", 1)
          assert "exit-codes-changing.service: error reloading" in out, out

      with subtest("activating the previous profile again exits 0"):
          activate("${toplevel}", 0)
          machine.wait_for_unit("exit-codes-changing.service")

      with subtest("a reload that fails exits 4 and is named"):
          out = activate("${failingReloadToplevel}", 4)
          assert "units failed after activation: exit-codes-reloading.service (job failed)" in out, out
          machine.succeed("systemctl is-active exit-codes-reloading.service")
          activate("${toplevel}", 0)

      with subtest("a unit whose start fails on an unmanaged dependency exits 4 and is named"):
          machine.succeed(
              "printf '[Service]\\nType=oneshot\\nExecStart=/bin/false\\n' > /run/systemd/system/exit-codes-unmanaged.service"
          )
          machine.succeed("systemctl daemon-reload")
          out = activate("${dependencyToplevel}", 4)
          assert "units failed after activation: exit-codes-dependent.service (job dependency)" in out, out
          machine.fail("systemctl is-active exit-codes-dependent.service")
          activate("${toplevel}", 0)
          machine.succeed("rm /run/systemd/system/exit-codes-unmanaged.service && systemctl daemon-reload")

      with subtest("a unit the profile drops that doesn't stop exits 1 and is named"):
          activate("${mountToplevel}", 0)
          machine.succeed("mountpoint -q /mnt/exitcodes")
          # A process working in the mount keeps it busy, without depending on it.
          machine.succeed("systemd-run --unit=exit-codes-busy /bin/sh -c 'cd /mnt/exitcodes && exec sleep infinity'")
          machine.wait_until_succeeds(
              "test \"$(readlink /proc/$(systemctl show -P MainPID exit-codes-busy.service)/cwd)\" = /mnt/exitcodes",
              timeout=30,
          )
          out = activate("${toplevel}", 1)
          assert "activation incomplete: " in out, out
          assert "mnt-exitcodes.mount not stopped (job failed)" in out, out
          machine.succeed("mountpoint -q /mnt/exitcodes")

      with subtest("activating that profile again retries the stop"):
          out = activate("${toplevel}", 1)
          assert "mnt-exitcodes.mount not stopped (job failed)" in out, out

      with subtest("a rollback to the profile that has the unit leaves it as it is"):
          since = machine.succeed("systemctl show -P ActiveEnterTimestampMonotonic mnt-exitcodes.mount")
          activate("${mountToplevel}", 0)
          machine.succeed("mountpoint -q /mnt/exitcodes")
          assert machine.succeed("systemctl show -P ActiveEnterTimestampMonotonic mnt-exitcodes.mount") == since
          activate("${toplevel}", 1)

      with subtest("once the stop can complete, activating again exits 0 and stops the unit"):
          machine.succeed("systemctl stop exit-codes-busy.service")
          activate("${toplevel}", 0)
          machine.fail("mountpoint -q /mnt/exitcodes")
    '';
}

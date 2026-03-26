{
  forEachDistro,
  system-manager,
  system,
  ...
}:

let
  configV2 = system-manager.lib.makeSystemConfig {
    modules = [
      {
        nixpkgs.hostPlatform = system;

        systemd.services.long-running-task = {
          description = "Long Running Task";
          wantedBy = [ "system-manager.target" ];
          restartIfChanged = false;
          serviceConfig.Type = "simple";
          serviceConfig.Environment = "FOO=v2";
          script = ''
            sleep infinity
          '';
        };
      }
    ];
  };
in

forEachDistro "restart-if-changed" {
  modules = [
    {
      systemd.services.long-running-task = {
        description = "Long Running Task";
        wantedBy = [ "system-manager.target" ];
        restartIfChanged = false;
        serviceConfig.Type = "simple";
        serviceConfig.Environment = "FOO=v1";
        script = ''
          sleep infinity
        '';
      };
    }
  ];
  extraPathsToRegister = [ configV2 ];
  testScriptFunction =
    { toplevel, hostPkgs, ... }:
    ''
      start_all()

      machine.wait_for_unit("multi-user.target")

      with subtest("activate configV1 and verify service is running"):
          activation_logs = machine.activate()
          machine.wait_for_unit("system-manager.target")
          result = machine.succeed("systemctl is-active long-running-task.service").strip()
          assert result == "active", f"Expected active, got {result}"

      with subtest("record initial PID"):
          pid_before = machine.succeed("systemctl show -p MainPID --value long-running-task.service").strip()
          assert pid_before != "0", "Service should have a non-zero PID"

      with subtest("activate configV2 with X-RestartIfChanged=false"):
          activation_logs = machine.activate(profile="${configV2}")
          machine.wait_for_unit("system-manager.target")

      with subtest("service was not restarted (same PID)"):
          pid_after = machine.succeed("systemctl show -p MainPID --value long-running-task.service").strip()
          assert pid_after == pid_before, \
              f"Service was restarted: PID changed from {pid_before} to {pid_after}"

      with subtest("activation logs show skip message"):
          assert "Skipping restart" in activation_logs and "long-running-task" in activation_logs, \
              f"Expected skip message in logs, got: {activation_logs}"
    '';
}

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
      }
    ];
  };
in

forEachDistro "stop-on-removal" {
  modules = [
    {
      systemd.services.long-running-task = {
        description = "Long Running Task";
        wantedBy = [ "system-manager.target" ];
        unitConfig.X-StopOnRemoval = false;
        serviceConfig.Type = "simple";
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
          machine.activate()
          machine.wait_for_unit("system-manager.target")
          result = machine.succeed("systemctl is-active long-running-task.service").strip()
          assert result == "active", f"Expected active, got {result}"

      with subtest("record initial PID"):
          pid_before = machine.succeed("systemctl show -p MainPID --value long-running-task.service").strip()
          assert pid_before != "0", "Service should have a non-zero PID"

      with subtest("activate configV2 with X-StopOnRemoval=false (service removed from config)"):
          activation_logs = machine.activate("${configV2}")

      with subtest("service was not stopped (process still alive)"):
          machine.succeed(f"test -d /proc/{pid_before}")

      with subtest("activation logs show skip message"):
          assert "Skipping stop" in activation_logs and "long-running-task" in activation_logs, \
              f"Expected skip message in logs, got: {activation_logs}"
    '';
}

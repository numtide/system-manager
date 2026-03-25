{ forEachDistro, ... }:

forEachDistro "auto-upgrade" {
  modules = [
    {
      system.autoUpgrade = {
        enable = true;
        flake = "github:example/repo";
        dates = "Mon 03:00";
        randomizedDelaySec = "45min";
        persistent = true;
        fixedRandomDelay = true;
      };
    }
  ];
  testScriptFunction =
    { toplevel, hostPkgs, ... }:
    ''
      start_all()

      machine.wait_for_unit("multi-user.target")

      activation_logs = machine.activate()
      for line in activation_logs.split("\n"):
          assert not "ERROR" in line, line
      machine.wait_for_unit("system-manager.target")

      with subtest("timer unit file exists"):
          timer = machine.file("/etc/systemd/system/system-manager-upgrade.timer")
          assert timer.exists, "system-manager-upgrade.timer should exist"

      with subtest("service unit file exists"):
          service = machine.file("/etc/systemd/system/system-manager-upgrade.service")
          assert service.exists, "system-manager-upgrade.service should exist"

      with subtest("timer is enabled via wantedBy symlink"):
          machine.succeed("test -L /etc/systemd/system/system-manager.target.wants/system-manager-upgrade.timer")

      with subtest("timer OnCalendar matches configured dates"):
          content = machine.succeed("cat /etc/systemd/system/system-manager-upgrade.timer")
          assert "OnCalendar=Mon 03:00" in content, \
              f"Expected 'OnCalendar=Mon 03:00' in timer, got: {content}"

      with subtest("deactivation removes the units"):
          machine.succeed("${toplevel}/bin/deactivate")
          timer = machine.file("/etc/systemd/system/system-manager-upgrade.timer")
          assert not timer.exists, "system-manager-upgrade.timer should be removed after deactivation"
          service = machine.file("/etc/systemd/system/system-manager-upgrade.service")
          assert not service.exists, "system-manager-upgrade.service should be removed after deactivation"
    '';
}

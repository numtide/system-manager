{ forEachDistro, ... }:

forEachDistro "masked-units" {
  modules = [
    (
      { ... }:
      {
        systemd.maskedUnits = [ "unattended-upgrades.service" ];
      }
    )
    ../../examples/example.nix
  ];
  testScriptFunction =
    { toplevel, hostPkgs, ... }:
    ''
      start_all()

      machine.wait_for_unit("multi-user.target")

      with subtest("Service is not masked before activation"):
          machine.fail("test -L /etc/systemd/system/unattended-upgrades.service")

      with subtest("Service can be started before activation"):
          assert machine.service("unattended-upgrades").is_running, "unattended-upgrades should be running before activation"

      machine.activate()
      machine.wait_for_unit("system-manager.target")

      with subtest("Masked service is not running"):
          assert not machine.service("unattended-upgrades").is_running, "unattended-upgrades should not be running"

      with subtest("Service is masked after activation"):
          resolved = machine.succeed("readlink -f /etc/systemd/system/unattended-upgrades.service").strip()
          assert resolved == "/dev/null", f"expected /dev/null, got {resolved}"

      with subtest("Masked service cannot be started"):
          machine.fail("systemctl start unattended-upgrades.service")

      with subtest("Deactivation unmasks the service"):
          machine.succeed("${toplevel}/bin/deactivate")
          machine.fail("test -L /etc/systemd/system/unattended-upgrades.service")
    '';
}

{ lib, forEachDistro, ... }:

forEachDistro "masked-units" {
  modules = [
    (
      { distroConfig, ... }:
      {
        systemd.maskedUnits = [ distroConfig.maskableService ];
      }
    )
    ../../examples/example.nix
  ];
  testScriptFunction =
    { toplevel, distroConfig, ... }:
    let
      unit = distroConfig.maskableService;
      service = lib.removeSuffix ".service" unit;
    in
    ''
      start_all()

      machine.wait_for_unit("multi-user.target")

      with subtest("Service is not masked before activation"):
          machine.fail("test -L /etc/systemd/system/${unit}")

      with subtest("Service can be started before activation"):
          assert machine.service("${service}").is_running, "${service} should be running before activation"

      machine.activate()
      machine.wait_for_unit("system-manager.target")

      with subtest("Masked service is not running"):
          assert not machine.service("${service}").is_running, "${service} should not be running"

      with subtest("Service is masked after activation"):
          resolved = machine.succeed("readlink -f /etc/systemd/system/${unit}").strip()
          assert resolved == "/dev/null", f"expected /dev/null, got {resolved}"

      with subtest("Masked service cannot be started"):
          machine.fail("systemctl start ${unit}")

      with subtest("Deactivation unmasks the service"):
          machine.succeed("${toplevel}/bin/deactivate")
          machine.fail("test -L /etc/systemd/system/${unit}")
    '';
}

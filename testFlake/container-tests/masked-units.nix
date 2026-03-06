{
  makeContainerTestFor,
  supportedDistros,
  ...
}:

let
  ubuntuDistros = builtins.filter (name: builtins.match "ubuntu.*" name != null) (
    builtins.attrNames supportedDistros
  );
in
builtins.listToAttrs (
  map (distroName: {
    name = "container-${distroName}-masked-units";
    value = makeContainerTestFor distroName supportedDistros.${distroName} "masked-units" {
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
        { toplevel, ... }:
        ''
          start_all()

          machine.wait_for_unit("multi-user.target")

          with subtest("Service is not masked before activation"):
              machine.fail("test -L /etc/systemd/system/unattended-upgrades.service")

          machine.activate()
          machine.wait_for_unit("system-manager.target")

          with subtest("Service is masked after activation"):
              resolved = machine.succeed("readlink -f /etc/systemd/system/unattended-upgrades.service").strip()
              assert resolved == "/dev/null", f"expected /dev/null, got {resolved}"

          with subtest("Masked service cannot be started"):
              machine.fail("systemctl start unattended-upgrades.service")

          with subtest("Deactivation unmasks the service"):
              machine.succeed("${toplevel}/bin/deactivate")
              machine.fail("test -L /etc/systemd/system/unattended-upgrades.service")
        '';
    };
  }) ubuntuDistros
)

{ forEachDistro, ... }:

forEachDistro "systemd-override-strategy" {
  modules = [
    (
      { pkgs, ... }:
      let
        etcOnlyUnit = pkgs.writeTextDir "etc/systemd/system/etc-only.service" ''
          [Unit]
          Description=Unit shipped from etc/systemd/system

          [Service]
          Type=oneshot
          ExecStart=/bin/sh -c 'touch /run/etc-only-service-started'
        '';

        asDropinBaseUnit = pkgs.writeTextDir "etc/systemd/system/as-dropin.service" ''
          [Unit]
          Description=Base unit from package (asDropin)

          [Service]
          Type=oneshot
          ExecStart=/bin/true
        '';

        asDropinIfExistsBaseUnit = pkgs.writeTextDir "lib/systemd/system/as-if-exists.service" ''
          [Unit]
          Description=Base unit from package (asDropinIfExists)

          [Service]
          Type=oneshot
          ExecStart=/bin/true
        '';
      in
      {
        systemd.packages = [
          etcOnlyUnit
          asDropinBaseUnit
          asDropinIfExistsBaseUnit
        ];

        systemd.services = {
          as-dropin = {
            enable = true;
            overrideStrategy = "asDropin";
            description = "Override generated as explicit drop-in";
            script = ''
              echo "as-dropin override"
            '';
          };

          as-if-exists = {
            enable = true;
            description = "Override generated as drop-in only if base unit exists";
            script = ''
              echo "as-if-exists override"
            '';
          };
        };
      }
    )
  ];
  testScriptFunction =
    { toplevel, hostPkgs, ... }:
    ''
      start_all()
      machine.wait_for_unit("multi-user.target")

      machine.activate()
      machine.wait_for_unit("system-manager.target")

      with subtest("Unit file from package etc/systemd/system is copied"):
          unit = machine.file("/etc/systemd/system/etc-only.service")
          assert unit.exists, "etc-only.service should exist"
          assert unit.is_symlink or unit.is_file, "etc-only.service should be a file or symlink"
          machine.succeed("systemctl start etc-only.service")
          machine.succeed("test -f /run/etc-only-service-started")

      with subtest("overrideStrategy=asDropin produces a drop-in"):
          machine.succeed("test -L /etc/systemd/system/as-dropin.service")
          machine.succeed("test -L /etc/systemd/system/as-dropin.service.d/overrides.conf")

      with subtest("default overrideStrategy behaves as asDropinIfExists"):
          machine.succeed("test -L /etc/systemd/system/as-if-exists.service")
          machine.succeed("test -L /etc/systemd/system/as-if-exists.service.d/overrides.conf")
    '';
}

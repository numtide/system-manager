{
  forEachDistro,
  ...
}:

forEachDistro "locale" {
  modules = [
    (
      { pkgs, ... }:
      {
        systemd.services.locale-probe = {
          enable = true;
          serviceConfig = {
            Type = "oneshot";
            RemainAfterExit = true;
            ExecStart = "${pkgs.coreutils}/bin/true";
          };
        };
      }
    )
  ];

  testScriptFunction =
    { ... }:
    ''
      start_all()

      machine.wait_for_unit("multi-user.target")

      machine.activate()
      machine.wait_for_unit("system-manager.target")

      with subtest("generated units get LOCALE_ARCHIVE and TZDIR"):
          unit = machine.succeed("cat /etc/systemd/system/locale-probe.service")
          assert "LOCALE_ARCHIVE=" in unit, unit
          assert "TZDIR=" in unit, unit

      with subtest("the referenced timezone directory exists"):
          tzdir = [line.split("TZDIR=")[1].rstrip('"') for line in unit.splitlines() if "TZDIR=" in line][0]
          machine.succeed(f"test -d {tzdir}")
    '';
}

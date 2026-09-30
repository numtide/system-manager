# A configuration must be able to point the locale variables elsewhere, and to
# opt out of the store locale archive entirely.
#
# The defaults in nix/modules/locale.nix are mkDefault, so a plain definition
# outranks them, and i18n.glibcLocales = null drops LOCALE_ARCHIVE rather than
# interpolating "null/lib/locale/locale-archive".
{
  forEachDistro,
  ...
}:

forEachDistro "locale-override" {
  modules = [
    (
      { pkgs, ... }:
      {
        # A plain definition, not mkDefault, so it wins over the module.
        systemd.globalEnvironment.TZDIR = "/sentinel/zoneinfo";

        # Opt out: no archive exists to point LOCALE_ARCHIVE at.
        i18n.glibcLocales = null;

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

      with subtest("a plain definition overrides the mkDefault TZDIR"):
          unit = machine.succeed("cat /etc/systemd/system/locale-probe.service")
          assert 'Environment="TZDIR=/sentinel/zoneinfo"' in unit, unit

      with subtest("i18n.glibcLocales = null drops LOCALE_ARCHIVE"):
          unit = machine.succeed("cat /etc/systemd/system/locale-probe.service")
          assert "LOCALE_ARCHIVE" not in unit, unit
    '';
}

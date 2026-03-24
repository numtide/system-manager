{
  forEachDistro,
  nixpkgs,
  lib,
  ...
}:

forEachDistro "systemd-packages" {
  modules = [
    (
      { pkgs, ... }:
      {
        imports = [ "${nixpkgs}/nixos/modules/services/security/fail2ban.nix" ];
        config = {

          # Enabling fail2ban to test systemd units overrides and
          # systemd.packages options.
          services.fail2ban = {
            enable = true;
            bantime = "3600";
            packageFirewall = pkgs.nftables;
          };
          networking.nftables.enable = true;
        };
        options = {
          # Some goes for nftables
          networking.nftables.enable = lib.mkEnableOption "dummy nftable module";
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

      with subtest("Unit file from systemd.packages is present"):
          unit = machine.file("/etc/systemd/system/fail2ban.service")
          assert unit.exists, "fail2ban.service unit file should exist"
          assert unit.is_symlink or unit.is_file, "fail2ban.service should be a file or symlink"
          machine.wait_for_unit("fail2ban.service")
    '';
}

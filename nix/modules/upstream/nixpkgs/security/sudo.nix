{
  config,
  lib,
  pkgs,
  pam-shim,
  ...
}:
{
  options.security.pam.services = lib.mkOption {
    type = lib.types.attrsOf lib.types.anything;
    default = { };
    internal = true;
  };

  config = lib.mkIf config.security.sudo.enable {
    environment.etc.sudoers.replaceExisting = true;

    # Use pam_shim to replace pam in the sudo package so the Nix-built
    # sudo binary delegates PAM calls to the host system's native libpam
    security.sudo.package = pkgs.sudo.override {
      pam = pam-shim;
    };

    # preserve compatibility with existing sudoers.d
    security.sudo.extraConfig = ''
      @includedir /etc/sudoers.d
    '';
  };
}

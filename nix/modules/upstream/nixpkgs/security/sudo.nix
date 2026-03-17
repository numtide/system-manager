{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.security.sudo;
in
{
  # Stub for PAM options referenced by upstream security/sudo.nix.
  # system-manager does not manage PAM configuration.
  options.security.pam.services = lib.mkOption {
    type = lib.types.attrsOf lib.types.anything;
    default = { };
    internal = true;
  };

  config = lib.mkIf cfg.enable {
    environment.etc.sudoers.replaceExisting = true;

    # Disable the SUID wrappers for sudo/sudoedit. The host's native
    # /usr/bin/sudo is already SUID and uses the host's PAM libraries.
    # Shipping a Nix-built sudo would break PAM on non-NixOS systems.
    security.wrappers.sudo.enable = false;
    security.wrappers.sudoedit.enable = false;

    # Replace the Nix-built sudo package with an empty stub so it does
    # not pollute systemPackages with PAM-incompatible binaries.
    security.sudo.package =
      pkgs.runCommand "sudo-host"
        {
          pname = "sudo";
        }
        ''
          mkdir -p $out
        '';

    # preserve compatibility with existing sudoers.d
    security.sudo.extraConfig = ''
      @includedir /etc/sudoers.d
    '';

    system-manager.preActivationAssertions.sudoInPath = {
      enable = true;
      script = ''
        if ! command -v sudo > /dev/null 2>&1; then
          echo "security.sudo is enabled but 'sudo' was not found in PATH."
          echo "Install sudo on the host system (e.g. apt install sudo) before activating."
          exit 1
        fi
      '';
    };
  };
}

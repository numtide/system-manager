{
  nixosModulesPath,
  lib,
  pkgs,
  ...
}:
{
  imports = [
    ./firewall.nix
    ./nginx.nix
    ./nix.nix
    ./programs/ssh.nix
    ./security-wrappers.nix
    ./security/sudo.nix
    ./userborn.nix
    ./users-groups.nix
    ../sops-nix.nix
    ./openssh.nix
  ]
  ++
    # List of imported NixOS modules
    # TODO: how will we manage this in the long term?
    map (path: nixosModulesPath + path) [
      "/misc/meta.nix"
      "/misc/ids.nix"
      "/security/acme/"
      "/security/sudo.nix"
      "/security/wrappers/"
      "/services/web-servers/nginx/"
      # nix settings
      "/config/nix.nix"
      "/services/system/userborn.nix"
      "/system/build.nix"
    ];

  options =
    # We need to ignore a bunch of options that are used in NixOS modules but
    # that don't apply to system-manager configs.
    # TODO: can we print an informational message for things like kernel modules
    # to inform users that they need to be enabled in the host system?
    {
      boot = lib.mkOption {
        type = lib.types.raw;
      };

      # nixos/modules/services/system/userborn.nix still depends on activation scripts
      # but just to verify that the "users" activation script is disabled.
      # We try to avoid having to import the whole activationScripts module.
      system.activationScripts.users = lib.mkOption {
        type = lib.types.str;
        default = "";
      };

      # Stubs for home-manager
      system.userActivationScripts = lib.mkOption {
        type = lib.types.attrsOf lib.types.unspecified;
        default = { };
      };

      # Stub, we don't import the display manager modules.
      services.displayManager.hiddenUsers = lib.mkOption {
        type = lib.types.listOf lib.types.str;
        default = [ ];
      };

      # Stub, we don't import the bash module.
      programs.bash.completion.enable = lib.mkOption {
        type = lib.types.bool;
        default = false;
      };

      fonts.fontconfig.enable = lib.mkOption {
        type = lib.types.bool;
        default = false;
      };

      # Only the locale archive is consumed (nix/modules/locale.nix sets
      # LOCALE_ARCHIVE from it), and pkgs.glibcLocales is built with
      # allLocales = true, which is a 223 MB locale-archive. Build just the two
      # locales NixOS enables by default instead, the same list as
      # nixos/modules/config/i18n.nix's i18n.supportedLocales. A configuration
      # that needs more can override this option with its own
      # pkgs.glibcLocales.override, or set it to null to get no archive at all.
      i18n.glibcLocales = lib.mkOption {
        type = lib.types.nullOr lib.types.package;
        default =
          if pkgs.glibcLocales != null then
            pkgs.glibcLocales.override {
              allLocales = false;
              locales = [
                "C.UTF-8/UTF-8"
                "en_US.UTF-8/UTF-8"
              ];
            }
          else
            null;
        defaultText = lib.literalExpression ''
          if pkgs.glibcLocales != null then
            pkgs.glibcLocales.override {
              allLocales = false;
              locales = [ "C.UTF-8/UTF-8" "en_US.UTF-8/UTF-8" ];
            }
          else
            null
        '';
      };
    };
}

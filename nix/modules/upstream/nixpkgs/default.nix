{
  nixosModulesPath,
  config,
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
      "/config/nix-channel.nix"
      "/config/nix-flakes.nix"
      "/config/nix-remote-build.nix"
      "/misc/nixpkgs-flake.nix"
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

      # nix-channel.nix registers a pre-switch check warning about leftover
      # channel state. We don't run NixOS pre-switch checks, so ignore them.
      system.preSwitchChecks = lib.mkOption {
        type = lib.types.attrsOf lib.types.raw;
        default = { };
      };

      # Channels are a NixOS-only concept here, see nix.nix for the rationale.
      nix.channel.enable = lib.mkOption {
        internal = true;
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
      # allLocales = true, which is a 223 MB locale-archive. The default below
      # reproduces nixos/modules/config/i18n.nix rather than hardcoding a list,
      # so the installed set is a real option: a host whose LANG is not in the
      # default pair (for example de_AT.UTF-8) declares it here, and "all"
      # installs every locale glibc ships. Setting i18n.glibcLocales directly
      # still bypasses this for a fully custom archive, or null for none.
      i18n.supportedLocales = lib.mkOption {
        type = lib.types.listOf lib.types.str;
        default = [
          "C.UTF-8/UTF-8"
          "en_US.UTF-8/UTF-8"
        ];
        example = [
          "en_US.UTF-8/UTF-8"
          "de_AT.UTF-8/UTF-8"
        ];
        description = ''
          Locales the generated locale archive should contain. The value
          `"all"` installs every locale glibc supports, which is a much larger
          archive. The default is the same pair NixOS enables by default.
        '';
      };

      i18n.glibcLocales = lib.mkOption {
        type = lib.types.nullOr lib.types.package;
        default =
          if pkgs.glibcLocales != null then
            pkgs.glibcLocales.override {
              allLocales = lib.elem "all" config.i18n.supportedLocales;
              locales = config.i18n.supportedLocales;
            }
          else
            null;
        defaultText = lib.literalExpression ''
          if pkgs.glibcLocales != null then
            pkgs.glibcLocales.override {
              allLocales = lib.elem "all" config.i18n.supportedLocales;
              locales = config.i18n.supportedLocales;
            }
          else
            null
        '';
        description = ''
          Customized `pkgs.glibcLocales`. Setting this directly bypasses
          `i18n.supportedLocales`.
        '';
      };
    };
}

{
  config,
  lib,
  pkgs,
  system-manager,
  ...
}:
let
  cfg = config.system.autoUpgrade;
in
{
  options.system.autoUpgrade = {

    enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Whether to periodically upgrade the system-manager configuration.
        When enabled, a systemd timer runs
        `system-manager switch --flake <uri>` on the configured schedule.
      '';
    };

    flake = lib.mkOption {
      type = lib.types.str;
      example = "github:numtide/example";
      description = ''
        The flake URI passed to `system-manager switch --flake`.
      '';
    };

    flags = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      example = [
        "--ephemeral"
        "--sudo"
      ];
      description = ''
        Additional flags passed to `system-manager switch`.
      '';
    };

    dates = lib.mkOption {
      type = lib.types.str;
      default = "04:40";
      example = "daily";
      description = ''
        How often or when the upgrade runs, in
        {manpage}`systemd.time(7)` calendar event format.
      '';
    };

    randomizedDelaySec = lib.mkOption {
      type = lib.types.str;
      default = "0";
      example = "45min";
      description = ''
        Random delay added before each upgrade, as a
        {manpage}`systemd.time(7)` time span.
      '';
    };

    fixedRandomDelay = lib.mkOption {
      type = lib.types.bool;
      default = false;
      example = true;
      description = ''
        Make the randomized delay consistent between runs.
      '';
    };

    persistent = lib.mkOption {
      type = lib.types.bool;
      default = true;
      example = false;
      description = ''
        If true, a missed timer trigger (e.g. system was off) fires
        immediately on next boot.
      '';
    };

    # No-op options for NixOS compatibility.
    # These allow configs that target both NixOS and system-manager

    allowReboot = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        This option has no effect in system-manager (no kernel/initrd
        to reboot into). It exists for NixOS configuration compatibility.
      '';
    };

    rebootWindow = lib.mkOption {
      type =
        with lib.types;
        nullOr (submodule {
          options = {
            lower = lib.mkOption {
              type = lib.types.strMatching "[[:digit:]]{2}:[[:digit:]]{2}";
              example = "01:00";
            };
            upper = lib.mkOption {
              type = lib.types.strMatching "[[:digit:]]{2}:[[:digit:]]{2}";
              example = "05:00";
            };
          };
        });
      default = null;
      description = ''
        This option has no effect in system-manager (no kernel/initrd
        to reboot into). It exists for NixOS configuration compatibility.
      '';
    };

    operation = lib.mkOption {
      type = lib.types.enum [
        "switch"
        "boot"
      ];
      default = "switch";
      description = ''
        This option has no effect in system-manager (only switch is
        supported). It exists for NixOS configuration compatibility.
      '';
    };

    channel = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = ''
        This option has no effect in system-manager (flake-only).
        It exists for NixOS configuration compatibility.
      '';
    };

    upgrade = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        This option has no effect in system-manager.
        It exists for NixOS configuration compatibility.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    warnings =
      lib.optional cfg.allowReboot "system.autoUpgrade.allowReboot has no effect: system-manager does not manage the kernel or initrd"
      ++
        lib.optional (cfg.rebootWindow != null)
          "system.autoUpgrade.rebootWindow has no effect: system-manager does not manage the kernel or initrd"
      ++ lib.optional (
        cfg.operation != "switch"
      ) "system.autoUpgrade.operation has no effect: system-manager only supports switch"
      ++ lib.optional (
        cfg.channel != null
      ) "system.autoUpgrade.channel has no effect: system-manager is flake-only"
      ++ lib.optional (!cfg.upgrade) "system.autoUpgrade.upgrade has no effect in system-manager";

    system.autoUpgrade.flags = [
      "--refresh"
      "--flake ${cfg.flake}"
    ];

    systemd.services.system-manager-upgrade = {
      description = "System Manager Upgrade";

      restartIfChanged = false;
      unitConfig.X-StopOnRemoval = false;

      serviceConfig.Type = "oneshot";

      path = with pkgs; [
        coreutils
        gitMinimal
      ];

      script = ''
        ${system-manager}/bin/system-manager switch ${lib.concatStringsSep " " cfg.flags}
      '';

      startAt = [ cfg.dates ];

      after = [ "network-online.target" ];
      wants = [ "network-online.target" ];
    };

    systemd.timers.system-manager-upgrade = {
      timerConfig = {
        RandomizedDelaySec = cfg.randomizedDelaySec;
        FixedRandomDelay = cfg.fixedRandomDelay;
        Persistent = cfg.persistent;
      };
    };
  };
}

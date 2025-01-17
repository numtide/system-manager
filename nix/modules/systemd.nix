{
  lib,
  config,
  pkgs,
  utils,
  ...
}:

let
  cfg = config.systemd;

  inherit (utils) systemdUtils;
  systemd-lib = utils.systemdUtils.lib;
in
{
  options.systemd = {

    # TODO: this is a bit dirty.
    # The value here gets added to the PATH of every service.
    # We could consider copying the systemd lib from NixOS and removing the bits
    # that are not relevant to us, like this option.
    package = lib.mkOption {
      type = lib.types.oneOf [
        lib.types.str
        lib.types.path
        lib.types.package
      ];
      default = pkgs.systemdMinimal;
    };

    globalEnvironment = lib.mkOption {
      type =
        with lib.types;
        attrsOf (
          nullOr (oneOf [
            str
            path
            package
          ])
        );
      default = { };
      example = {
        TZ = "CET";
      };
      description = lib.mdDoc ''
        Environment variables passed to *all* systemd units.
      '';
    };

    enableStrictShellChecks = lib.mkEnableOption "running shellcheck on the generated scripts for systemd units.";

    units = lib.mkOption {
      description = lib.mdDoc "Definition of systemd units.";
      default = { };
      type = systemdUtils.types.units;
    };

    packages = lib.mkOption {
      default = [ ];
      type = lib.types.listOf lib.types.package;
      example = lib.literalExpression "[ pkgs.systemd-cryptsetup-generator ]";
      description = lib.mdDoc "Packages providing systemd units and hooks.";
    };

    targets = lib.mkOption {
      default = { };
      type = systemdUtils.types.targets;
      description = lib.mdDoc "Definition of systemd target units.";
    };

    services = lib.mkOption {
      default = { };
      type = systemdUtils.types.services;
      description = lib.mdDoc "Definition of systemd service units.";
    };

    sockets = lib.mkOption {
      default = { };
      type = systemdUtils.types.sockets;
      description = lib.mdDoc "Definition of systemd socket units.";
    };

    timers = lib.mkOption {
      default = { };
      type = systemdUtils.types.timers;
      description = lib.mdDoc "Definition of systemd timer units.";
    };

    paths = lib.mkOption {
      default = { };
      type = systemdUtils.types.paths;
      description = lib.mdDoc "Definition of systemd path units.";
    };

    mounts = lib.mkOption {
      default = [ ];
      type = systemdUtils.types.mounts;
      description = lib.mdDoc ''
        Definition of systemd mount units.
        This is a list instead of an attrSet, because systemd mandates the names to be derived from
        the 'where' attribute.
      '';
    };

    automounts = lib.mkOption {
      default = [ ];
      type = systemdUtils.types.automounts;
      description = lib.mdDoc ''
        Definition of systemd automount units.
        This is a list instead of an attrSet, because systemd mandates the names to be derived from
        the 'where' attribute.
      '';
    };

    slices = lib.mkOption {
      default = { };
      type = systemdUtils.types.slices;
      description = lib.mdDoc "Definition of slice configurations.";
    };

    generators = lib.mkOption {
      type = lib.types.attrsOf lib.types.path;
      default = { };
      example = {
        systemd-gpt-auto-generator = "/dev/null";
      };
      description = lib.mdDoc ''
        Definition of systemd generators.
        For each `NAME = VALUE` pair of the attrSet, a link is generated from
        `/etc/systemd/system-generators/NAME` to `VALUE`.
      '';
    };

    shutdown = lib.mkOption {
      type = lib.types.attrsOf lib.types.path;
      default = { };
      description = lib.mdDoc ''
        Definition of systemd shutdown executables.
        For each `NAME = VALUE` pair of the attrSet, a link is generated from
        `/etc/systemd/system-shutdown/NAME` to `VALUE`.
      '';
    };
  };

  config = {
    systemd = {
      targets.system-manager = {
        wantedBy = [ "default.target" ];
      };

      timers = lib.mapAttrs (name: service: {
        wantedBy = [ "timers.target" ];
        timerConfig.OnCalendar = service.startAt;
      }) (lib.filterAttrs (name: service: service.enable && service.startAt != [ ]) cfg.services);

      units =
        lib.mapAttrs' (n: v: lib.nameValuePair "${n}.path" (systemd-lib.pathToUnit v)) cfg.paths
        // lib.mapAttrs' (n: v: lib.nameValuePair "${n}.service" (systemd-lib.serviceToUnit v)) cfg.services
        // lib.mapAttrs' (n: v: lib.nameValuePair "${n}.slice" (systemd-lib.sliceToUnit v)) cfg.slices
        // lib.mapAttrs' (n: v: lib.nameValuePair "${n}.socket" (systemd-lib.socketToUnit v)) cfg.sockets
        // lib.mapAttrs' (n: v: lib.nameValuePair "${n}.target" (systemd-lib.targetToUnit v)) cfg.targets
        // lib.mapAttrs' (n: v: lib.nameValuePair "${n}.timer" (systemd-lib.timerToUnit v)) cfg.timers
        // lib.listToAttrs (
          map (
            v:
            let
              n = utils.escapeSystemdPath v.where;
            in
            lib.nameValuePair "${n}.mount" (systemd-lib.mountToUnit v)
          ) cfg.mounts
        )
        // lib.listToAttrs (
          map (
            v:
            let
              n = utils.escapeSystemdPath v.where;
            in
            lib.nameValuePair "${n}.automount" (systemd-lib.automountToUnit v)
          ) cfg.automounts
        );
    };

    environment.etc =
      let
        enabledUnits = lib.filterAttrs (_: unit: unit.enable) cfg.units;
      in
      {
        "systemd/system".source =
          let
            # The default value of the `package` parameter of
            # `systemd-lib.generateUnits` copies a number of unit files and
            # `.wants` links out of the package passed as the value of the
            # `package` parameter (by default, `config.systemd.package`).
            # This copying is liable to conflict with existing units and
            # `.wants` links on the target system, and may trigger other
            # issues, so pass a package that contains nothing.
            empty = pkgs.runCommand "empty-directory" { } ''
              mkdir -p $out
            '';
          in
          systemd-lib.generateUnits {
            inherit (cfg) packages;

            package = empty;
            units = enabledUnits;
            upstreamUnits = [ ];
            upstreamWants = [ ];

            # Don't link misc. stuff like `default.target`; otherwise act like
            # `type = "system"`.
            type = "initrd";
          };
      };
  };
}

{ lib
, config
, pkgs
, utils
, ...
}:

let
  cfg = config.systemd;

  inherit (utils) systemdUtils;
  systemd-lib = utils.systemdUtils.lib;

  generateUnits = { allowCollisions ? true, type, units, packages ? cfg.packages }:
    pkgs.runCommand "${type}-units"
      { preferLocalBuild = true;
        allowSubstitutes = false;
      } ''
        mkdir -p $out

        # Symlink all units provided listed in systemd.packages.
        packages="${toString packages}"

        # Filter duplicate directories
        declare -A unique_packages
        for k in $packages ; do unique_packages[$k]=1 ; done

        for i in ''${!unique_packages[@]}; do
          for fn in $i/etc/systemd/${type}/* $i/lib/systemd/${type}/*; do
            if ! [[ "$fn" =~ .wants$ ]]; then
              if [[ -d "$fn" ]]; then
                targetDir="$out/$(basename "$fn")"
                mkdir -p "$targetDir"
                ${pkgs.buildPackages.xorg.lndir}/bin/lndir "$fn" "$targetDir"
              else
                ln -s $fn $out/
              fi
            fi
          done
        done

        for i in ${toString (lib.mapAttrsToList (n: v: v.unit) units)}; do
          fn=$(basename $i/*)
          if [ -e $out/$fn ]; then
            if [ "$(readlink -f $i/$fn)" = /dev/null ]; then
              ln -sfn /dev/null $out/$fn
            else
              ${if allowCollisions then ''
                mkdir -p $out/$fn.d
                ln -s $i/$fn $out/$fn.d/overrides.conf
              '' else ''
                echo "Found multiple derivations configuring $fn!"
                exit 1
              ''}
            fi
          else
            ln -fs $i/$fn $out/
          fi
        done

        ${lib.concatStrings (
          lib.mapAttrsToList (name: unit:
            lib.concatMapStrings (name2: ''
              mkdir -p $out/'${name2}.wants'
              ln -sfn '../${name}' $out/'${name2}.wants'/
            '') (unit.wantedBy or [])
          ) units)}

        ${lib.concatStrings (
          lib.mapAttrsToList (name: unit:
            lib.concatMapStrings (name2: ''
              mkdir -p $out/'${name2}.requires'
              ln -sfn '../${name}' $out/'${name2}.requires'/
            '') (unit.requiredBy or [])
          ) units)}
      '';
in
{
  options.systemd = {

    # TODO: this is a bit dirty.
    # The value here gets added to the PATH of every service.
    # We could consider copying the systemd lib from NixOS and removing the bits
    # that are not relevant to us, like this option.
    package = lib.mkOption {
      type = lib.types.oneOf [ lib.types.str lib.types.path lib.types.package ];
      default = pkgs.systemdMinimal;
    };

    globalEnvironment = lib.mkOption {
      type = with lib.types; attrsOf (nullOr (oneOf [ str path package ]));
      default = { };
      example = { TZ = "CET"; };
      description = lib.mdDoc ''
        Environment variables passed to *all* systemd units.
      '';
    };

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
      example = { systemd-gpt-auto-generator = "/dev/null"; };
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

  options.systemd.user = {
    units = lib.mkOption {
      description = lib.mdDoc "Definition of systemd per-user units.";
      default = {};
      type = systemdUtils.types.units;
    };

    paths = lib.mkOption {
      default = {};
      type = systemdUtils.types.paths;
      description = lib.mdDoc "Definition of systemd per-user path units.";
    };

    services = lib.mkOption {
      default = {};
      type = systemdUtils.types.services;
      description = lib.mdDoc "Definition of systemd per-user service units.";
    };

    slices = lib.mkOption {
      default = {};
      type = systemdUtils.types.slices;
      description = lib.mdDoc "Definition of systemd per-user slice units.";
    };

    sockets = lib.mkOption {
      default = {};
      type = systemdUtils.types.sockets;
      description = lib.mdDoc "Definition of systemd per-user socket units.";
    };

    targets = lib.mkOption {
      default = {};
      type = systemdUtils.types.targets;
      description = lib.mdDoc "Definition of systemd per-user target units.";
    };

    timers = lib.mkOption {
      default = {};
      type = systemdUtils.types.timers;
      description = lib.mdDoc "Definition of systemd per-user timer units.";
    };
  };

  config = {
    systemd = {
      targets.system-manager = {
        wantedBy = [ "default.target" ];
      };

      timers =
        lib.mapAttrs
          (name: service:
            {
              wantedBy = [ "timers.target" ];
              timerConfig.OnCalendar = service.startAt;
            })
          (lib.filterAttrs (name: service: service.enable && service.startAt != [ ]) cfg.services);

      units =
        lib.mapAttrs' (n: v: lib.nameValuePair "${n}.path" (systemd-lib.pathToUnit v)) cfg.paths
        // lib.mapAttrs' (n: v: lib.nameValuePair "${n}.service" (systemd-lib.serviceToUnit v)) cfg.services
        // lib.mapAttrs' (n: v: lib.nameValuePair "${n}.slice" (systemd-lib.sliceToUnit v)) cfg.slices
        // lib.mapAttrs' (n: v: lib.nameValuePair "${n}.socket" (systemd-lib.socketToUnit v)) cfg.sockets
        // lib.mapAttrs' (n: v: lib.nameValuePair "${n}.target" (systemd-lib.targetToUnit v)) cfg.targets
        // lib.mapAttrs' (n: v: lib.nameValuePair "${n}.timer" (systemd-lib.timerToUnit v)) cfg.timers
        // lib.listToAttrs (map
          (v:
            let n = utils.escapeSystemdPath v.where;
            in lib.nameValuePair "${n}.mount" (systemd-lib.mountToUnit v))
          cfg.mounts)
        // lib.listToAttrs (map
          (v:
            let n = utils.escapeSystemdPath v.where;
            in lib.nameValuePair "${n}.automount" (systemd-lib.automountToUnit v))
          cfg.automounts);
    };

    systemd.user = {
      timers =
        lib.mapAttrs
          (name: service:
            {
              wantedBy = [ "timers.target" ];
              timerConfig.OnCalendar = service.startAt;
            })
          (lib.filterAttrs (name: service: service.enable && service.startAt != [ ]) cfg.user.services);

      units =
        lib.mapAttrs' (n: v: lib.nameValuePair "${n}.path" (systemd-lib.pathToUnit n v)) cfg.user.paths
        // lib.mapAttrs' (n: v: lib.nameValuePair "${n}.service" (systemd-lib.serviceToUnit n v)) cfg.user.services
        // lib.mapAttrs' (n: v: lib.nameValuePair "${n}.slice" (systemd-lib.sliceToUnit n v)) cfg.user.slices
        // lib.mapAttrs' (n: v: lib.nameValuePair "${n}.socket" (systemd-lib.socketToUnit n v)) cfg.user.sockets
        // lib.mapAttrs' (n: v: lib.nameValuePair "${n}.target" (systemd-lib.targetToUnit n v)) cfg.user.targets
        // lib.mapAttrs' (n: v: lib.nameValuePair "${n}.timer" (systemd-lib.timerToUnit n v)) cfg.user.timers;
    };

    environment.etc = {
      "systemd/system".source = generateUnits {
        type = "system";
        units = lib.filterAttrs (_: unit: unit.enable) cfg.units;
      };

      "systemd/user".source = generateUnits {
        type = "user";
        units = lib.filterAttrs (_: unit: unit.enable) cfg.user.units;
      };
    };
  };
}

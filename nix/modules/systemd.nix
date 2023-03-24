{ lib
, config
, pkgs
, utils
, ...
}:

let
  cfg = config.system-manager.systemd;

  inherit (utils) systemdUtils;
  systemd-lib = utils.systemdUtils.lib;
in
{
  options.system-manager.systemd = {

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

  config = {
    system-manager = {
      systemd = {
        timers =
          lib.mapAttrs
            (name: service:
              {
                wantedBy = [ "timers.target" ];
                timerConfig.OnCalendar = service.startAt;
              })
            (lib.filterAttrs (name: service: service.enable && service.startAt != [ ]) cfg.services);

        units =
          lib.mapAttrs' (n: v: lib.nameValuePair "${n}.path" (systemd-lib.pathToUnit n v)) cfg.paths
          // lib.mapAttrs' (n: v: lib.nameValuePair "${n}.service" (systemd-lib.serviceToUnit n v)) cfg.services
          // lib.mapAttrs' (n: v: lib.nameValuePair "${n}.slice" (systemd-lib.sliceToUnit n v)) cfg.slices
          // lib.mapAttrs' (n: v: lib.nameValuePair "${n}.socket" (systemd-lib.socketToUnit n v)) cfg.sockets
          // lib.mapAttrs' (n: v: lib.nameValuePair "${n}.target" (systemd-lib.targetToUnit n v)) cfg.targets
          // lib.mapAttrs' (n: v: lib.nameValuePair "${n}.timer" (systemd-lib.timerToUnit n v)) cfg.timers
          // lib.listToAttrs (map
            (v:
              let n = utils.escapeSystemdPath v.where;
              in lib.nameValuePair "${n}.mount" (systemd-lib.mountToUnit n v))
            cfg.mounts)
          // lib.listToAttrs (map
            (v:
              let n = utils.escapeSystemdPath v.where;
              in lib.nameValuePair "${n}.automount" (systemd-lib.automountToUnit n v))
            cfg.automounts);
      };

      environment.etc =
        let
          allowCollisions = false;

          enabledUnits = cfg.units;
        in
        {
          "systemd/system".source = pkgs.runCommand "system-manager-units"
            {
              preferLocalBuild = true;
              allowSubstitutes = false;
            }
            ''
              mkdir -p $out

              for i in ${toString (lib.mapAttrsToList (n: v: v.unit) enabledUnits)}; do
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
                ) enabledUnits)}

              ${lib.concatStrings (
                lib.mapAttrsToList (name: unit:
                  lib.concatMapStrings (name2: ''
                    mkdir -p $out/'${name2}.requires'
                    ln -sfn '../${name}' $out/'${name2}.requires'/
                  '') (unit.requiredBy or [])
                ) enabledUnits)}
            '';
        };
    };
  };
}

{
  lib,
  config,
  options,
  pkgs,
  ...
}:

{
  options.environment = {
    systemPackages = lib.mkOption {
      type = lib.types.listOf lib.types.package;
      default = [ ];
    };

    corePackages = lib.mkOption {
      type = lib.types.listOf lib.types.package;
      default = [ ];
      description = ''
        Packages that are considered essential for the system to function.
        These are automatically included in `environment.systemPackages`.
        NixOS modules use this to register packages they depend on.
      '';
    };

    pathsToLink = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
    };

    extraInit = lib.mkOption {
      type = lib.types.lines;
      default = "";
      description = "Shell script code which should be called before any shell session through the host /etc/profile.";
    };

    variables = lib.mkOption {
      default = { };
      example = {
        EDITOR = "nvim";
        VISUAL = "nvim";
      };
      description = ''
        A set of environment variables used in the global environment.
        These variables will be set on shell initialisation (e.g. in /etc/profile).

        The value of each variable can be either a string or a list of
        strings.  The latter is concatenated, interspersed with colon
        characters.

        Setting a variable to `null` does nothing. You can override a
        variable set by another module to `null` to unset it.
      '';
      type =
        with lib.types;
        attrsOf (
          nullOr (oneOf [
            (listOf (oneOf [
              int
              str
              path
            ]))
            int
            str
            path
          ])
        );
      apply =
        let
          toStr = v: if lib.isPath v then "${v}" else toString v;
        in
        attrs:
        lib.mapAttrs (_: v: if lib.isList v then lib.concatMapStringsSep ":" toStr v else toStr v) (
          lib.filterAttrs (_: v: v != null) attrs
        );
    };

    sessionVariables = lib.mkOption {
      default = { };
      description = ''
        A set of environment variables used in the global environment.
        These variables will be set by PAM early in the login process.

        The value of each session variable can be either a string or a
        list of strings. The latter is concatenated, interspersed with
        colon characters.

        Setting a variable to `null` does nothing. You can override a
        variable set by another module to `null` to unset it.

        Note: unlike NixOS, system-manager does not manage PAM on the
        host, so these variables are not injected by pam_env into
        non-shell sessions (e.g. graphical logins).
      '';
      inherit (options.environment.variables) type apply;
    };
  };

  config =
    let
      pathDir = "/run/system-manager/sw";
    in
    {
      environment = {
        systemPackages = config.environment.corePackages;

        pathsToLink = [
          "/bin"
        ];

        variables = config.environment.sessionVariables;

        etc = {
          "profile.d/system-manager-path.sh".source = pkgs.writeText "system-manager-path.sh" ''
            ${lib.concatLines (lib.mapAttrsToList (k: v: ''export ${k}="${v}"'') config.environment.variables)}
            export PATH=${pathDir}/bin:''${PATH}
            ${config.environment.extraInit}
          '';

          # TODO: figure out how to properly add fish support. We could start by
          # looking at what NixOS and HM do to set up the fish env.
          #"fish/conf.d/system-manager-path.fish".source =
          #  pkgs.writeTextFile {
          #    name = "system-manager-path.fish";
          #    executable = true;
          #    text = ''
          #      set -gx PATH "${pathDir}/bin/" $PATH
          #    '';
          #  };
        };
      };

      systemd.services.system-manager-path = {
        enable = true;
        description = "";
        wantedBy = [ "system-manager.target" ];
        serviceConfig = {
          Type = "oneshot";
          RemainAfterExit = true;
        };
        script =
          let
            pathDrv = pkgs.buildEnv {
              name = "system-manager-path";
              paths = config.environment.systemPackages;
              inherit (config.environment) pathsToLink;
            };
          in
          ''
            mkdir --parents $(dirname "${pathDir}")
            if [ -L "${pathDir}" ]; then
              unlink "${pathDir}"
            fi
            ln --symbolic --force "${pathDrv}" "${pathDir}"
          '';
      };
    };
}

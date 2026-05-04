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

    extraOutputsToInstall = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      description = ''
        List of additional package outputs to be symlinked into
        `/run/system-manager/sw` and per-user profiles.
      '';
    };

    extraInit = lib.mkOption {
      type = lib.types.lines;
      default = "";
      description = "Shell script code which should be called before any shell session through the host /etc/profile.";
    };

    extraSetup = lib.mkOption {
      type = lib.types.lines;
      default = "";
      description = "Shell fragments to be run after the system environment has been created. This should only be used for things that need to modify the internals of the environment, e.g. generating MIME caches. The environment being built can be accessed at $out.";
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

        Unlike [](#opt-environment.variables), session variables will
        append the existing value of the variable using the
        `''${parameter:+word}` shell expansion. For example, setting
        `XDG_DATA_DIRS` to `"/nix/share"` will produce
        `export XDG_DATA_DIRS="/nix/share''${XDG_DATA_DIRS:+:''$XDG_DATA_DIRS}"`,
        which preserves any pre-existing value.

        Note: unlike NixOS, system-manager does not manage PAM on the
        host, so these variables are not injected by pam_env into
        non-shell sessions (e.g. graphical logins).
      '';
      inherit (options.environment.variables) type apply;
    };
  };

  options.system.path = lib.mkOption {
    type = lib.types.package;
    internal = true;
    description = ''
      The top-level system environment derivation, combining
      `environment.systemPackages` into a single buildEnv. Exposed so
      that modules copied verbatim from NixOS (e.g. `users-groups.nix`)
      can reference `config.system.path.{ignoreCollisions,postBuild}`.
    '';
  };

  config =
    let
      pathDir = "/run/system-manager/sw";

      sessionVarNames = builtins.attrNames config.environment.sessionVariables;

      exportLine =
        k: v:
        if builtins.elem k sessionVarNames then
          "export ${k}=\"" + v + "$\{" + k + ":+:$" + k + "}\""
        else
          "export ${k}=\"" + v + "\"";
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
            ${lib.concatLines (lib.mapAttrsToList exportLine config.environment.variables)}
            export PATH=${pathDir}/bin:''${PATH}
            if [ -d "/etc/profiles/per-user/$USER/bin" ]; then
              export PATH="/etc/profiles/per-user/$USER/bin:$PATH"
            fi
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

      system.path = pkgs.buildEnv {
        name = "system-manager-path";
        paths = config.environment.systemPackages;
        inherit (config.environment) pathsToLink extraOutputsToInstall;
        ignoreCollisions = true;
        postBuild = config.environment.extraSetup;
      };

      systemd.services.system-manager-path = {
        enable = true;
        description = "";
        wantedBy = [ "system-manager.target" ];
        serviceConfig = {
          Type = "oneshot";
          RemainAfterExit = true;
        };
        script = ''
          mkdir --parents $(dirname "${pathDir}")
          if [ -L "${pathDir}" ]; then
            unlink "${pathDir}"
          fi
          ln --symbolic --force "${config.system.path}" "${pathDir}"
        '';
      };
    };
}

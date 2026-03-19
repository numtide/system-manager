# Vendored from nixos/modules/programs/ssh.nix with system-manager adaptations:
# - Added programs.ssh.enable option (defaults to false)
# - Guarded etc file generation behind enable flag
# - Disabled features not applicable to non-NixOS systems
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.programs.ssh;

  knownHosts = builtins.attrValues cfg.knownHosts;

  knownHostsText =
    (lib.flip (lib.concatMapStringsSep "\n") knownHosts (
      h:
      assert h.hostNames != [ ];
      lib.optionalString h.certAuthority "@cert-authority "
      + builtins.concatStringsSep "," h.hostNames
      + " "
      + (if h.publicKey != null then h.publicKey else builtins.readFile h.publicKeyFile)
    ))
    + "\n";

  knownHostsFiles = [ "/etc/ssh/ssh_known_hosts" ] ++ map pkgs.copyPathToStore cfg.knownHostsFiles;
in
{
  options = {
    # Stubs for options referenced by this module but not available in
    # system-manager.
    services.xserver.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      internal = true;
    };

    environment.variables = lib.mkOption {
      type = lib.types.attrsOf lib.types.str;
      default = { };
      internal = true;
    };

    systemd.user.services = lib.mkOption {
      type = lib.types.attrs;
      default = { };
      internal = true;
    };

    programs.ssh = {
      enable = lib.mkOption {
        type = lib.types.bool;
        default = false;
        description = ''
          Whether to manage SSH client configuration.
          When disabled, system-manager will not touch existing
          `/etc/ssh/ssh_config` or `/etc/ssh/ssh_known_hosts`.
        '';
      };

      enableAskPassword = lib.mkOption {
        type = lib.types.bool;
        default = false;
        description = "Whether to configure SSH_ASKPASS in the environment.";
      };

      systemd-ssh-proxy.enable = lib.mkOption {
        type = lib.types.bool;
        default = false;
        description = ''
          Whether to enable systemd's ssh proxy plugin.
          See {manpage}`systemd-ssh-proxy(1)`.
        '';
      };

      askPassword = lib.mkOption {
        type = lib.types.str;
        default = "${pkgs.x11_ssh_askpass}/libexec/x11-ssh-askpass";
        defaultText = lib.literalExpression ''"''${pkgs.x11_ssh_askpass}/libexec/x11-ssh-askpass"'';
        description = "Program used by SSH to ask for passwords.";
      };

      forwardX11 = lib.mkOption {
        type = with lib.types; nullOr bool;
        default = false;
        description = ''
          Whether to request X11 forwarding on outgoing connections by default.
          If set to null, the option is not set at all.
        '';
      };

      setXAuthLocation = lib.mkOption {
        type = lib.types.bool;
        default = false;
        description = ''
          Whether to set the path to {command}`xauth` for X11-forwarded connections.
          This causes a dependency on X11 packages.
        '';
      };

      pubkeyAcceptedKeyTypes = lib.mkOption {
        type = lib.types.listOf lib.types.str;
        default = [ ];
        example = [
          "ssh-ed25519"
          "ssh-rsa"
        ];
        description = ''
          Specifies the key types that will be used for public key authentication.
        '';
      };

      hostKeyAlgorithms = lib.mkOption {
        type = lib.types.listOf lib.types.str;
        default = [ ];
        example = [
          "ssh-ed25519"
          "ssh-rsa"
        ];
        description = ''
          Specifies the host key algorithms that the client wants to use in order of preference.
        '';
      };

      extraConfig = lib.mkOption {
        type = lib.types.lines;
        default = "";
        description = ''
          Extra configuration text prepended to {file}`ssh_config`. Other generated
          options will be added after a `Host *` pattern.
          See {manpage}`ssh_config(5)` for help.
        '';
      };

      startAgent = lib.mkOption {
        type = lib.types.bool;
        default = false;
        description = ''
          Whether to start the OpenSSH agent when you log in.
        '';
      };

      agentTimeout = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        example = "1h";
        description = ''
          How long to keep the private keys in memory. Use null to keep them forever.
        '';
      };

      agentPKCS11Whitelist = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        description = ''
          A pattern-list of acceptable paths for PKCS#11 shared libraries
          that may be used with the -s option to ssh-add.
        '';
      };

      package = lib.mkPackageOption pkgs "openssh" { };

      knownHosts = lib.mkOption {
        default = { };
        type = lib.types.attrsOf (
          lib.types.submodule (
            {
              name,
              config,
              options,
              ...
            }:
            {
              options = {
                certAuthority = lib.mkOption {
                  type = lib.types.bool;
                  default = false;
                  description = ''
                    This public key is an SSH certificate authority, rather than an
                    individual host's key.
                  '';
                };
                hostNames = lib.mkOption {
                  type = lib.types.listOf lib.types.str;
                  default = [ name ] ++ config.extraHostNames;
                  defaultText = lib.literalExpression "[ ${name} ] ++ config.${options.extraHostNames}";
                  description = ''
                    A list of host names and/or IP numbers used for accessing
                    the host's ssh service.
                  '';
                };
                extraHostNames = lib.mkOption {
                  type = lib.types.listOf lib.types.str;
                  default = [ ];
                  description = ''
                    A list of additional host names and/or IP numbers used for
                    accessing the host's ssh service.
                  '';
                };
                publicKey = lib.mkOption {
                  default = null;
                  type = lib.types.nullOr lib.types.str;
                  example = "ecdsa-sha2-nistp521 AAAAE2VjZHN...UEPg==";
                  description = ''
                    The public key data for the host.
                  '';
                };
                publicKeyFile = lib.mkOption {
                  default = null;
                  type = lib.types.nullOr lib.types.path;
                  description = ''
                    The path to the public key file for the host.
                  '';
                };
              };
            }
          )
        );
        description = ''
          The set of system-wide known SSH hosts.
        '';
      };

      knownHostsFiles = lib.mkOption {
        default = [ ];
        type = with lib.types; listOf path;
        description = ''
          Files containing SSH host keys to set as global known hosts.
        '';
      };

      kexAlgorithms = lib.mkOption {
        type = lib.types.nullOr (lib.types.listOf lib.types.str);
        default = null;
        description = ''
          Specifies the available KEX (Key Exchange) algorithms.
        '';
      };

      ciphers = lib.mkOption {
        type = lib.types.nullOr (lib.types.listOf lib.types.str);
        default = null;
        description = ''
          Specifies the ciphers allowed and their order of preference.
        '';
      };

      macs = lib.mkOption {
        type = lib.types.nullOr (lib.types.listOf lib.types.str);
        default = null;
        description = ''
          Specifies the MAC (message authentication code) algorithms in order of preference.
        '';
      };
    };
  };

  config = lib.mkIf cfg.enable {
    services.openssh.settings.X11Forwarding = lib.mkDefault false;

    assertions = lib.flip lib.mapAttrsToList cfg.knownHosts (
      name: data: {
        assertion =
          (data.publicKey == null && data.publicKeyFile != null)
          || (data.publicKey != null && data.publicKeyFile == null);
        message = "knownHost ${name} must contain either a publicKey or publicKeyFile";
      }
    );

    environment.corePackages = [ cfg.package ];

    environment.etc."ssh/ssh_config" = {
      replaceExisting = true;
      text = lib.concatStringsSep "\n" (
        lib.optional (cfg.extraConfig != "") cfg.extraConfig
        ++ [
          ''
            # Generated options from other settings
            Host *
          ''
        ]
        ++ [
          "GlobalKnownHostsFile ${builtins.concatStringsSep " " knownHostsFiles}"
        ]
        ++ lib.optional (!config.networking.enableIPv6) "AddressFamily inet"
        ++ lib.optional cfg.setXAuthLocation "XAuthLocation ${pkgs.xauth}/bin/xauth"
        ++ lib.optional (cfg.forwardX11 != null) "ForwardX11 ${lib.boolToYesNo cfg.forwardX11}"
        ++ lib.optional (
          cfg.pubkeyAcceptedKeyTypes != [ ]
        ) "PubkeyAcceptedKeyTypes ${builtins.concatStringsSep "," cfg.pubkeyAcceptedKeyTypes}"
        ++ lib.optional (
          cfg.hostKeyAlgorithms != [ ]
        ) "HostKeyAlgorithms ${builtins.concatStringsSep "," cfg.hostKeyAlgorithms}"
        ++ lib.optional (
          cfg.kexAlgorithms != null
        ) "KexAlgorithms ${builtins.concatStringsSep "," cfg.kexAlgorithms}"
        ++ lib.optional (cfg.ciphers != null) "Ciphers ${builtins.concatStringsSep "," cfg.ciphers}"
        ++ lib.optional (cfg.macs != null) "MACs ${builtins.concatStringsSep "," cfg.macs}"
      );
    };

    environment.etc."ssh/ssh_known_hosts" = {
      replaceExisting = true;
      text = knownHostsText;
    };

    environment.extraInit = lib.optionalString cfg.startAgent ''
      if [ -z "$SSH_AUTH_SOCK" -a -n "$XDG_RUNTIME_DIR" ]; then
        export SSH_AUTH_SOCK="$XDG_RUNTIME_DIR/ssh-agent"
      fi
    '';

    environment.variables.SSH_ASKPASS = lib.optionalString cfg.enableAskPassword cfg.askPassword;
  };
}

# Compatibility layer for the sops-nix NixOS module.
#
# system-manager has no equivalent of the NixOS activation-script machinery: a
# configuration is activated by writing units under /etc/systemd/system and
# restarting sysinit-reactivation.target. sops-nix already supports that model
# through `sops.useSystemdActivation`, which installs secrets from a
# sops-install-secrets.service oneshot instead of from the `setupSecrets`
# activation script.
#
# This module pins that code path on and patches up the places where sops-nix's
# systemd path still assumes it is running on NixOS. The activation-script
# options below are still declared, because sops-nix writes to them on the code
# paths we do not take, and because we lift the age key generation script out of
# one of them.
{
  lib,
  config,
  options,
  pkgs,
  ...
}:
let
  # sops-nix is an optional import: everything below has to degrade to a no-op
  # when the user has not imported it.
  sopsImported = options ? sops.secrets;

  cfg = config.sops or { };
  secrets = cfg.secrets or { };
  secretsForUsers = lib.filterAttrs (_: secret: secret.neededForUsers) secrets;

  # sops-nix assigns `lib.stringAfter [ ] "..."` here, i.e. an attrset with a
  # `text` attribute. The stub default is the empty string.
  generateAgeKeyScript =
    let
      value = config.system.activationScripts.generate-age-key;
    in
    if lib.isAttrs value then value.text else value;
in
{
  options.system.activationScripts = {
    generate-age-key = lib.mkOption {
      type = lib.types.raw;
      default = "";
    };
    setupSecrets = lib.mkOption {
      type = lib.types.raw;
      default = "";
    };
    setupSecretsForUsers = lib.mkOption {
      type = lib.types.raw;
      default = "";
    };
  };

  config = lib.optionalAttrs sopsImported (
    lib.mkMerge [
      {
        # sops-nix only turns this on by default when sysusers or userborn manage
        # users. system-manager cannot run activation scripts at all, so the
        # systemd path is the only one that can ever install secrets here:
        # disabling userborn must not silently turn secrets management off.
        sops.useSystemdActivation = lib.mkDefault true;

        assertions = [
          {
            assertion = secrets != { } -> cfg.useSystemdActivation;
            message = ''
              sops.useSystemdActivation must be enabled: system-manager does not run
              NixOS activation scripts, so sops-nix would never install any secret.
            '';
          }
          {
            assertion = secretsForUsers != { } -> config.services.userborn.enable;
            message = ''
              sops.secrets.<name>.neededForUsers requires services.userborn.enable:
              sops-nix only emits the sops-install-secrets-for-users unit when
              userborn (or systemd-sysusers, which system-manager does not use)
              manages users. Affected secrets: ${lib.concatStringsSep ", " (lib.attrNames secretsForUsers)}
            '';
          }
        ];
      }

      (lib.mkIf (secretsForUsers != { } && config.services.userborn.enable) {
        # sops-nix orders this unit around systemd-sysusers.service, which
        # system-manager never runs. Re-point it at userborn, which is what
        # creates users here, and hook it into the reactivation target so that it
        # also runs on `system-manager switch` and not only on boot.
        systemd.services.sops-install-secrets-for-users = {
          wantedBy = lib.mkForce [ "sysinit.target" ];
          requiredBy = [ "sysinit-reactivation.target" ];
          before = lib.mkForce [
            "userborn.service"
            "sysinit-reactivation.target"
          ];
          after = [ "local-fs.target" ];
        };
      })

      (lib.mkIf (secrets != { } && generateAgeKeyScript != "") {
        # `sops.age.generateKey` is only implemented as an activation script
        # upstream. Run the very same script from a oneshot unit ordered before
        # the units that need the key, instead of discarding it.
        systemd.services.sops-generate-age-key = {
          description = "Generate a machine-specific age key for sops-nix";
          wantedBy = [ "sysinit.target" ];
          requiredBy = [ "sysinit-reactivation.target" ];
          after = [ "local-fs.target" ];
          before = [
            "sops-install-secrets.service"
            "sops-install-secrets-for-users.service"
            "sysinit-reactivation.target"
          ];
          unitConfig.DefaultDependencies = "no";
          serviceConfig = {
            Type = "oneshot";
            RemainAfterExit = true;
            ExecStart = pkgs.writeShellScript "sops-generate-age-key" generateAgeKeyScript;
          };
        };
      })
    ]
  );
}

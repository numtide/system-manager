# Stubs for sops-nix module compatibility.
# sops-nix uses activation scripts which system-manager does not support.
# These stubs allow importing the sops-nix module without errors.
{ lib, ... }:
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
}

# Test sops.secrets.<name>.neededForUsers.
#
# Upstream sops-nix orders sops-install-secrets-for-users.service around
# systemd-sysusers.service, which system-manager never runs: it creates users
# with userborn. system-manager re-points that unit at userborn and hooks it
# into sysinit-reactivation.target, so the secret is available by the time the
# user is created.
{
  forEachDistro,
  sops-nix,
  ...
}:

let
  # Matches the plaintext of `user-password` in ../sops/secrets-for-users.yaml.
  passwordHash = "$6$systemmgr$A2h1izy08/Rf3.0PVfEiHodQpxQuwD.92FriFKsKZWTm2jMbb4phE2rXaQzS1wCUABiajbeBfnDWFwr1IgwjX/";
in

forEachDistro "sops-needed-for-users" {
  modules = [
    (
      { config, ... }:
      {
        imports = [ sops-nix.nixosModules.sops ];

        config = {
          services.userborn.enable = true;

          sops = {
            age = {
              generateKey = false;
              keyFile = "/run/age-keys.txt";
            };
            defaultSopsFile = ../sops/secrets-for-users.yaml;
            secrets.user-password.neededForUsers = true;
          };

          users.users.sopsuser = {
            isNormalUser = true;
            hashedPasswordFile = config.sops.secrets.user-password.path;
          };
        };
      }
    )
  ];
  extraPathsToRegister = [ ../sops/age-keys.txt ];
  testScriptFunction =
    { ... }:
    ''
      start_all()

      machine.wait_for_unit("multi-user.target")
      machine.succeed("cp ${../sops/age-keys.txt} /run/age-keys.txt")

      machine.activate()

      machine.succeed("systemctl is-active sops-install-secrets-for-users.service")

      # The secret lands in /run/secrets-for-users, not /run/secrets.
      secret_value = machine.succeed("cat /run/secrets-for-users/user-password").strip()
      assert secret_value == "${passwordHash}", f"Unexpected secret contents: {secret_value}"
      machine.fail("test -e /run/secrets/user-password")

      # It is ordered before userborn, which is what consumes it.
      before = machine.succeed("systemctl show -p Before --value sops-install-secrets-for-users.service")
      assert "userborn.service" in before, f"Expected ordering before userborn.service, got: {before}"

      # And userborn actually picked the decrypted hash up.
      shadow_entry = machine.succeed("grep '^sopsuser:' /etc/shadow").strip()
      assert "${passwordHash}" in shadow_entry, f"Expected the sops-provided hash in /etc/shadow, got: {shadow_entry}"
    '';
}

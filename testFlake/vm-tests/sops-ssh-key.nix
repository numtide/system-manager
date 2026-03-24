# Test sops secrets decryption using SSH host key (age.sshKeyPaths)
# This verifies that secrets can be decrypted using an ed25519 SSH key
# converted to age format, which is useful for machines that already have
# SSH host keys and don't want to manage separate age keys.
{
  forEachUbuntuImage,
  testModule,
  system-manager,
  system,
  sops-nix,
  ...
}:
let
  # Config for SSH key-based sops decryption test
  sshKeyConfig = system-manager.lib.makeSystemConfig {
    modules = [
      (testModule "ssh")
      (
        { lib, pkgs, ... }:
        {
          imports = [ sops-nix.nixosModules.sops ];

          config = {
            nixpkgs.hostPlatform = system;

            services.nginx.enable = false;
            services.userborn.enable = true;

            sops = {
              # Use SSH key instead of age key file
              age.sshKeyPaths = [ "/etc/ssh/ssh_host_ed25519_key" ];
              defaultSopsFile = ../sops/secrets-ssh.yaml;
              secrets.test = { };
            };
            systemd.services.sops-install-secrets = {
              before = [ "sysinit-reactivation.target" ];
              requiredBy = [ "sysinit-reactivation.target" ];
            };
          };
        }
      )
    ];
  };
in
forEachUbuntuImage "sops-ssh-key" {
  modules = [
    (testModule "old")
    ../../examples/example.nix
  ];
  extraPathsToRegister = [
    sshKeyConfig
    ../sops/ssh-ed25519-key
  ];
  testScriptFunction =
    { toplevel, hostPkgs, ... }:
    ''
      start_all()

      vm.wait_for_unit("default.target")

      # Set up the SSH ed25519 key as if it were a host key
      vm.succeed("mkdir -p /etc/ssh")
      vm.succeed("cp ${../sops/ssh-ed25519-key} /etc/ssh/ssh_host_ed25519_key")
      vm.succeed("chmod 600 /etc/ssh/ssh_host_ed25519_key")

      # Activate the config that uses SSH key for sops decryption
      ${system-manager.lib.activateProfileSnippet {
        node = "vm";
        profile = sshKeyConfig;
      }}

      # Verify the secret was decrypted correctly
      secret_value = vm.succeed("cat /run/secrets/test").strip()
      assert secret_value == "itworks-ssh", f"Expected 'itworks-ssh', got '{secret_value}'"

      print("SSH key-based sops decryption test passed!")
    '';
}

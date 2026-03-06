{
  forEachDistro,
  system-manager,
  system,
  sops-nix,
  ...
}:

let
  newConfig = system-manager.lib.makeSystemConfig {
    modules = [
      (
        { lib, pkgs, ... }:
        {
          imports = [ sops-nix.nixosModules.sops ];
          config = {
            nixpkgs.hostPlatform = system;

            services.nginx.enable = false;

            environment = {
              etc = {
                foo_new = {
                  text = ''
                    This is just a test!
                  '';
                };
              };

              systemPackages = [
                pkgs.fish
              ];
            };

            systemd.services = {
              new-service = {
                enable = true;
                description = "new-service";
                serviceConfig = {
                  Type = "oneshot";
                  RemainAfterExit = true;
                  ExecReload = "${lib.getBin pkgs.coreutils}/bin/true";
                };
                wantedBy = [
                  "system-manager.target"
                  "default.target"
                ];
                script = ''
                  sleep 2
                '';
              };
            };

            nix = {
              enable = true;
              settings = {
                experimental-features = [
                  "nix-command"
                  "flakes"
                ];
                trusted-users = [ "zimbatm" ];
              };
            };

            users.users.zimbatm = {
              isNormalUser = true;
              extraGroups = [
                "wheel"
                "sudo"
              ];
              initialPassword = "test123";
            };

            sops = {
              age.generateKey = false;
              age.keyFile = "/run/age-keys.txt";
              defaultSopsFile = ../sops/secrets.yaml;
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

forEachDistro "system-path" {
  modules = [
    ../../examples/example.nix
  ];
  extraPathsToRegister = [ newConfig ];
  testScriptFunction =
    { toplevel, hostPkgs, ... }:
    ''
      # Start the container
      start_all()
      machine.wait_for_unit("multi-user.target")

      machine.fail("bash --login -c '$(command -v rg)'")
      machine.fail("bash --login -c '$(command -v fd)'")

      machine.activate()

      machine.wait_for_unit("system-manager.target")
      machine.wait_for_unit("system-manager-path.service")

      machine.succeed("bash --login -c 'realpath $(command -v rg) | grep -F ${hostPkgs.ripgrep}/bin/rg'")
      machine.succeed("bash --login -c 'realpath $(command -v fd) | grep -F ${hostPkgs.fd}/bin/fd'")

      machine.activate("${newConfig}")

      machine.fail("bash --login -c '$(command -v rg)'")
      machine.fail("bash --login -c '$(command -v fd)'")
      machine.succeed("bash --login -c 'realpath $(command -v fish) | grep -F ${hostPkgs.fish}/bin/fish'")
    '';
}

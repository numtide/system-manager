{
  lib,
  system-manager,
  system,
  nix-vm-test,
  sops-nix,
}:

let
  forEachUbuntuImage =
    name:
    {
      modules,
      testScriptFunction,
      extraPathsToRegister ? [ ],
      projectTest ? test: test.sandboxed,
    }:
    let
      ubuntu = nix-vm-test.ubuntu;
    in
    lib.listToAttrs (
      # Ubuntu 20.04 reaches end of life April 2025; drop support.
      lib.flip map (lib.filter (v: v != "20_04") (lib.attrNames ubuntu.images)) (
        imageVersion:
        let
          toplevel = (
            system-manager.lib.makeSystemConfig {
              modules = modules ++ [
                (
                  { lib, pkgs, ... }:
                  {
                    options.hostPkgs = lib.mkOption {
                      type = lib.types.raw;
                      readOnly = true;
                    };
                    config = {
                      nixpkgs.hostPlatform = system;
                      hostPkgs = pkgs;
                    };
                  }
                )
              ];
            }
          );
          inherit (toplevel.config) hostPkgs;
        in
        lib.nameValuePair "ubuntu-${imageVersion}-${name}" (
          projectTest (
            ubuntu.${imageVersion} {
              testScript = testScriptFunction { inherit toplevel hostPkgs; };
              extraPathsToRegister = extraPathsToRegister ++ [
                toplevel
              ];
              sharedDirs = { };
            }
          )
        )
      )
    );

  # To test reload and restart, we include two services, one that can be reloaded
  # and one that cannot.
  # The id parameter is a string that can be used to force reloading the services
  # between two configs by changing their contents.
  testModule =
    id:
    { lib, pkgs, ... }:
    {
      systemd.services = {
        has-reload = {
          enable = true;
          description = "service-reload";
          serviceConfig = {
            Type = "oneshot";
            RemainAfterExit = true;
            ExecReload = ''
              ${lib.getBin pkgs.coreutils}/bin/true
            '';
          };
          wantedBy = [ "system-manager.target" ];
          script = ''
            echo "I can be reloaded (id: ${id})"
          '';
        };
        has-no-reload = {
          enable = true;
          description = "service-no-reload";
          serviceConfig.Type = "simple";
          wantedBy = [ "system-manager.target" ];
          script = ''
            while true; do
              echo "I cannot be reloaded (id: ${id})"
            done
          '';
        };
      };
    };

  newConfig = system-manager.lib.makeSystemConfig {
    modules = [
      (testModule "new")
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

  callTest =
    file:
    import file {
      inherit
        forEachUbuntuImage
        testModule
        newConfig
        system-manager
        system
        lib
        sops-nix
        nix-vm-test
        ;
    };

  testFiles = lib.filterAttrs (name: type: name != "default.nix" && lib.hasSuffix ".nix" name) (
    builtins.readDir ./.
  );
in
lib.foldlAttrs (
  acc: name: _:
  acc // callTest (./. + "/${name}")
) { } testFiles

{
  lib,
  system-manager,
  system,
  nix-vm-test,
  sops-nix,
}:

let
  imageNames = builtins.attrNames (
    builtins.fromJSON (builtins.readFile ../../lib/container-test-driver/images.json)
  );

  forEachImage =
    name:
    {
      modules,
      testScriptFunction,
      extraPathsToRegister ? [ ],
      projectTest ? test: test.sandboxed,
    }:
    let
      toplevel = system-manager.lib.makeSystemConfig {
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
      };
      inherit (toplevel.config) hostPkgs;
    in
    lib.listToAttrs (
      map (
        image:
        let
          parts = lib.splitString "-" image;
        in
        lib.nameValuePair "vm-${image}-${name}" (
          projectTest (
            nix-vm-test.${lib.head parts}.${lib.last parts} {
              testScript = testScriptFunction { inherit toplevel hostPkgs; };
              extraPathsToRegister = extraPathsToRegister ++ [
                toplevel
              ];
              sharedDirs = { };
            }
          )
        )
      ) imageNames
    );

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

  callTest =
    file:
    import file {
      inherit
        forEachImage
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

{
  lib,
  system-manager,
  system,
  nix-vm-test,
  sops-nix,
}:

let
  distros = {
    ubuntu = {
      # Ubuntu 20.04 reaches end of life April 2025; drop support.
      filter = v: v != "20_04";
    };
    debian = {
      # Only Debian 13 (trixie)
      filter = v: v == "13";
    };
    fedora = {
      # Only Fedora 41
      filter = v: v == "41";
    };
  };

  forEachImage =
    name:
    {
      modules,
      testScriptFunction,
      extraPathsToRegister ? [ ],
      projectTest ? test: test.sandboxed,
    }:
    let
      mkToplevel = system-manager.lib.makeSystemConfig {
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
      mkTestForDistro =
        distroName: distroConfig:
        let
          distro = nix-vm-test.${distroName};
          versions = lib.filter distroConfig.filter (lib.attrNames distro.images);
        in
        lib.listToAttrs (
          map (
            imageVersion:
            let
              toplevel = mkToplevel;
              inherit (toplevel.config) hostPkgs;
            in
            lib.nameValuePair "vm-${distroName}-${imageVersion}-${name}" (
              projectTest (
                distro.${imageVersion} {
                  testScript = testScriptFunction { inherit toplevel hostPkgs; };
                  extraPathsToRegister = extraPathsToRegister ++ [
                    toplevel
                  ];
                  sharedDirs = { };
                }
              )
            )
          ) versions
        );
    in
    lib.foldlAttrs (
      acc: distroName: distroConfig:
      acc // mkTestForDistro distroName distroConfig
    ) { } distros;

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

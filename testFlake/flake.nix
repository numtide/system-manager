{
  description = "System Manager VM integration tests";

  nixConfig = {
    extra-substituters = [ "https://cache.numtide.com" ];
    extra-trusted-public-keys = [ "niks3.numtide.com-1:DTx8wZduET09hRmMtKdQDxNNthLQETkc/yaX7M4qK0g=" ];
  };

  inputs = {
    system-manager.url = "path:..";
    nixpkgs.follows = "system-manager/nixpkgs";
    nix-vm-test = {
      url = "github:numtide/nix-vm-test";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    sops-nix.url = "github:Mic92/sops-nix";
    sops-nix.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs =
    {
      self,
      system-manager,
      nixpkgs,
      nix-vm-test,
      sops-nix,
    }:
    let
      testedSystems = [
        "aarch64-linux"
        "x86_64-linux"
      ];

      # VM tests only run on x86_64-linux for now
      vmTestSystem = "x86_64-linux";
      vmTestLib = import "${nix-vm-test}/lib.nix" {
        inherit nixpkgs;
        system = vmTestSystem;
      };
      vmChecks =
        system:
        import ./vm-tests.nix {
          system = vmTestSystem;
          inherit (nixpkgs) lib;
          nix-vm-test = vmTestLib;
          inherit system-manager;
          inherit sops-nix;
        };
      containerChecks =
        system:
        import ./container-tests.nix {
          inherit system;
          inherit (nixpkgs) lib;
          hostPkgs = nixpkgs.legacyPackages.${system};
          inherit system-manager;
        };
    in
    {
      checks = nixpkgs.lib.genAttrs testedSystems (
        system:
        system-manager.checks.${system}
        // nixpkgs.lib.optionalAttrs (system == vmTestSystem) (vmChecks system)
        // (containerChecks system)
      );
    };
}

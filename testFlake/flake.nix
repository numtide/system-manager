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
  };

  outputs =
    {
      self,
      system-manager,
      nixpkgs,
      nix-vm-test,
    }:
    let
      # All systems supported by system-manager
      systems = [
        "aarch64-linux"
        "aarch64-darwin"
        "x86_64-linux"
      ];

      # VM tests only run on x86_64-linux (requires KVM)
      vmTestSystem = "x86_64-linux";
      vmTestLib = import "${nix-vm-test}/lib.nix" {
        inherit nixpkgs;
        system = vmTestSystem;
      };
      vmChecks = import ./vm-tests.nix {
        system = vmTestSystem;
        inherit (nixpkgs) lib;
        nix-vm-test = vmTestLib;
        system-manager = system-manager;
      };
    in
    {
      checks = nixpkgs.lib.genAttrs systems (
        system:
        system-manager.checks.${system} // nixpkgs.lib.optionalAttrs (system == vmTestSystem) vmChecks
      );
    };
}

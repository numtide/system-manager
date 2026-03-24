{
  description = "NixOS System Manager configuration";

  nixConfig = {
    extra-substituters = [ "https://cache.numtide.com" ];
    extra-trusted-public-keys = [ "niks3.numtide.com-1:DTx8wZduET09hRmMtKdQDxNNthLQETkc/yaX7M4qK0g=" ];
  };

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    system-manager.url = "github:numtide/system-manager";
  };

  outputs =
    { nixpkgs, system-manager, ... }:
    {
      nixosConfigurations.default = nixpkgs.lib.nixosSystem {
        system = "x86_64-linux";
        modules = [
          ./configuration.nix
          ./system.nix
          system-manager.nixosModules.system-manager
        ];
      };
    };
}

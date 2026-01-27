{
  description = "Standalone System Manager configuration";

  nixConfig = {
    extra-substituters = [ "https://cache.numtide.com" ];
    extra-trusted-public-keys = [ "niks3.numtide.com-1:DTx8wZduET09hRmMtKdQDxNNthLQETkc/yaX7M4qK0g=" ];
  };

  inputs = {
    # Specify the source of System Manager and Nixpkgs.
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    system-manager.url = "github:numtide/system-manager";
  };

  outputs =
    {
      self,
      nixpkgs,
      system-manager,
      ...
    }:
    let
      system = "x86_64-linux";
    in
    {
      systemConfigs.default = system-manager.lib.makeSystemConfig {
        # Specify your system configuration modules here, for example,
        # the path to your system.nix.
        modules = [ ./system.nix ];

        # Optionally specify extraSpecialArgs and overlays
      };
    };
}

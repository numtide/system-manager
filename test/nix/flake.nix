{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";

    system-manager = {
      url = "sourcehut:~r-vdp/system-manager";

      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-utils.follows = "flake-utils";
    };
  };

  outputs =
    { flake-utils
    , system-manager
    , ...
    }:
    {
      systemConfigs.default = system-manager.lib.makeSystemConfig {
        system = flake-utils.lib.system.x86_64-linux;
        modules = [
          ./modules
        ];
      };
    };
}

{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    system-manager = {
      url = "github:numtide/system-manager";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { system-manager, ... }:
    {
      systemConfigs.default = system-manager.lib.makeSystemConfig {
        modules = [
          ./modules
        ];
      };
    };
}

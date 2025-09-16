{
  description = "Manage system config using nix on any distro";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  inputs.system-manager.url = "../.";
  inputs.system-manager.inputs.nixpkgs.follows = "nixpkgs";
  inputs.sops-nix.url = "github:Mic92/sops-nix";
  inputs.sops-nix.inputs.nixpkgs.follows = "nixpkgs";

  outputs =
    {
      self,
      nixpkgs,
      sops-nix,
      system-manager,
    }@inputs:
    {
      systemConfigs.default = system-manager.lib.makeSystemConfig {
        modules = [ ./examples/example.nix ];
        extraSpecialArgs = {
          inherit inputs;
        };
      };
    };
}

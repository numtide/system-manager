{
  description = "Manage system config using nix on any distro";

  inputs.system-manager.url = "../.";
  inputs.sops-nix.url = "github:Mic92/sops-nix";
  inputs.sops-nix.inputs.nixpkgs.follows = "system-manager/nixpkgs";
  inputs.nix-vm-test.url = "github:numtide/nix-vm-test";
  inputs.nix-vm-test.inputs.nixpkgs.follows = "system-manager/nixpkgs";

  outputs =
    {
      self,
      nixpkgs,
      sops-nix,
      system-manager,
      nix-vm-test,
    }@inputs:
    {
      systemConfigs.default = system-manager.lib.makeSystemConfig {
        modules = [ ./example.nix ];
        extraSpecialArgs = { inherit inputs; };
      };

      checks = {
        x86_64-linux =
          let
            system = "x86_64-linux";
          in
          (import ./test/nix/modules {
            inherit system-manager;
            inherit system;
            inherit inputs;
            inherit (nixpkgs) lib;
            nix-vm-test = nix-vm-test.lib.${system};
          });
      };
    };
}

{
  description = "Manage system config using nix on any distro";

  inputs.system-manager.url = "../.";
  inputs.nix-vm-test.url = "github:numtide/nix-vm-test";
  inputs.nix-vm-test.inputs.nixpkgs.follows = "system-manager/nixpkgs";

  outputs =
    {
      self,
      nixpkgs,
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

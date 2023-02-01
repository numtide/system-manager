{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }: {
    serviceConfig = self.lib.makeServiceConfig {
      system = flake-utils.lib.system.x86_64-linux;
      module = { imports = [ ./nix/modules ]; };
    };

    lib = import ./nix/lib.nix { inherit nixpkgs; };
  };
}

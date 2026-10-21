final: _prev: {
  system-manager-unwrapped = final.callPackage ../../package.nix { };
  system-manager = final.callPackage ../packages/wrapper.nix {
    inherit (final) system-manager-unwrapped;
  };
}

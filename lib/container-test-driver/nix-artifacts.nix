{
  system,
}:
let
  nixVersion = "2.33.0";
in
{
  nix-installer =
    if system == "x86_64-linux" then
      builtins.fetchurl {
        url = "https://github.com/NixOS/nix-installer/releases/download/${nixVersion}/nix-installer-x86_64-linux";
        sha256 = "sha256-+GTcBIJ56ulEaP/xja+oLajdGb+bHDka9WQkU4XIMNM=";
      }
    else if system == "aarch64-linux" then
      builtins.fetchurl {
        url = "https://github.com/NixOS/nix-installer/releases/download/${nixVersion}/nix-installer-aarch64-linux";
        sha256 = "sha256-ociEB/P9kJAzUSxQCLmqOJEQpGuqvTQk+cEVtG6YIS4=";
      }
    else
      throw "Unsupported system: ${system}";

  nixTarball =
    if system == "x86_64-linux" then
      builtins.fetchurl {
        url = "https://releases.nixos.org/nix/nix-${nixVersion}/nix-${nixVersion}-x86_64-linux.tar.xz";
        sha256 = "00cgpm2l3mcmxqwvsvak0qwd498x9azm588czb5p3brmcvin3bsl";
      }
    else if system == "aarch64-linux" then
      builtins.fetchurl {
        url = "https://releases.nixos.org/nix/nix-${nixVersion}/nix-${nixVersion}-aarch64-linux.tar.xz";
        sha256 = "1v3z0qdfm6sa053qn39ijn2g9vsh1nrhykwsxx7piwlnvysn4hsw";
      }
    else
      throw "Unsupported system: ${system}";
}

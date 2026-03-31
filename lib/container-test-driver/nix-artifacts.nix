{
  system,
}:
let
  data = builtins.fromJSON (builtins.readFile ./nix-artifacts.json);
  nixVersion = data.nixVersion;
in
{
  nix-installer =
    if system == "x86_64-linux" then
      builtins.fetchurl {
        url = "https://github.com/NixOS/nix-installer/releases/download/${nixVersion}/nix-installer-x86_64-linux";
        sha256 = data.nix-installer.x86_64-linux;
      }
    else if system == "aarch64-linux" then
      builtins.fetchurl {
        url = "https://github.com/NixOS/nix-installer/releases/download/${nixVersion}/nix-installer-aarch64-linux";
        sha256 = data.nix-installer.aarch64-linux;
      }
    else
      throw "Unsupported system: ${system}";
}

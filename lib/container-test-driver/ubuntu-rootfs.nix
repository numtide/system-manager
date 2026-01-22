{
  pkgs,
  system ? pkgs.stdenv.hostPlatform.system,
}:
let
  nixVersion = "2.33.0";

  cloudImg =
    if system == "x86_64-linux" then
      builtins.fetchurl {
        url = "https://cloud-images.ubuntu.com/releases/noble/release-20251026/ubuntu-24.04-server-cloudimg-amd64-root.tar.xz";
        sha256 = "0y3d55f5qy7bxm3mfmnxzpmwp88d7iiszc57z5b9npc6xgwi28np";
      }
    else if system == "aarch64-linux" then
      builtins.fetchurl {
        url = "https://cloud-images.ubuntu.com/releases/noble/release-20251026/ubuntu-24.04-server-cloudimg-arm64-root.tar.xz";
        sha256 = "1l4l0llfffspzgnmwhax0fcnjn8ih8n4azhfaghng2hh1xvr4a17";
      }
    else
      throw "Unsupported system: ${system}";

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
in
pkgs.runCommand "ubuntu-rootfs-base"
  {
    nativeBuildInputs = [ pkgs.xz ];
  }
  ''
    mkdir -p $out

    # Extract Ubuntu cloud image
    tar --exclude='dev/*' \
        --exclude='etc/systemd/system/network-online.target.wants/*' \
        --exclude='etc/systemd/system/multi-user.target.wants/systemd-resolved.service' \
        --exclude='usr/lib/systemd/system/tpm-udev.service' \
        --exclude='usr/lib/systemd/system/systemd-remount-fs.service' \
        --exclude='usr/lib/systemd/system/systemd-resolved.service' \
        --exclude='usr/lib/systemd/system/proc-sys-fs-binfmt_misc.automount' \
        --exclude='usr/lib/systemd/system/sys-kernel-*' \
        -xJf ${cloudImg} -C $out

    # Remove existing symlinks if present and create FHS compatibility symlinks
    rm -f $out/bin $out/lib $out/lib64 $out/sbin
    ln -sf usr/bin $out/bin
    ln -sf usr/lib $out/lib
    ln -sf usr/lib64 $out/lib64 2>/dev/null || true
    ln -sf usr/sbin $out/sbin

    # Container marker for systemd
    mkdir -p $out/run/systemd
    echo 'systemd-nspawn' > $out/run/systemd/container

    # Include nix-installer binary
    mkdir -p $out/usr/local/bin
    install -m755 ${nix-installer} $out/usr/local/bin/nix-installer

    # Include Nix tarball for offline installation
    mkdir -p $out/usr/local/share/nix
    cp ${nixTarball} $out/usr/local/share/nix/nix.tar.xz

    # Create marker to indicate Nix needs installation
    touch $out/.nix-not-installed

    # Create var/lib/apt/lists for apt compatibility
    mkdir -p $out/var/lib/apt/lists/partial
  ''

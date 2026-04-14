{
  pkgs,
  system ? pkgs.stdenv.hostPlatform.system,
}:
let
  makeRootfs = import ./make-rootfs.nix { inherit pkgs system; };

  ubuntuExcludePatterns = [
    "etc/systemd/system/network-online.target.wants/*"
    "etc/systemd/system/multi-user.target.wants/systemd-resolved.service"
    "usr/lib/systemd/system/tpm-udev.service"
    "usr/lib/systemd/system/systemd-remount-fs.service"
    "usr/lib/systemd/system/systemd-resolved.service"
    "usr/lib/systemd/system/proc-sys-fs-binfmt_misc.automount"
    "usr/lib/systemd/system/sys-kernel-*"
    "usr/lib/systemd/system/snapd*"
    "usr/lib/systemd/system/snap-*"
    "etc/systemd/system/snapd*"
    "etc/systemd/system/snap-*"
    "etc/systemd/system/multi-user.target.wants/snap*"
    "etc/systemd/system/sockets.target.wants/snapd*"
  ];
in
{
  ubuntu-22_04 = {
    systems = [
      "x86_64-linux"
      "aarch64-linux"
    ];
    rootfs = makeRootfs.buildRootfs {
      name = "ubuntu-22_04";
      cloudImg =
        if system == "x86_64-linux" then
          builtins.fetchurl {
            url = "https://cloud-images.ubuntu.com/releases/jammy/release-20260227/ubuntu-22.04-server-cloudimg-amd64-root.tar.xz";
            sha256 = "05gw1sspv9d4m5yazc8105yc2vr3y9xkwnwilnzn774w9nivwib3";
          }
        else if system == "aarch64-linux" then
          builtins.fetchurl {
            url = "https://cloud-images.ubuntu.com/releases/jammy/release-20260227/ubuntu-22.04-server-cloudimg-arm64-root.tar.xz";
            sha256 = "1aya4ainn5289bhczbx97dxv7ck8ng3kmz8yiicz8ynvyfg6mvrq";
          }
        else
          throw "Unsupported system: ${system}";
      excludePatterns = ubuntuExcludePatterns;
      extraDirs = [ "var/lib/apt/lists/partial" ];
    };
    maskableService = "unattended-upgrades.service";
  };

  ubuntu-24_04 = {
    systems = [
      "x86_64-linux"
      "aarch64-linux"
    ];
    rootfs = makeRootfs.buildRootfs {
      name = "ubuntu-24_04";
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
      excludePatterns = ubuntuExcludePatterns;
      extraDirs = [ "var/lib/apt/lists/partial" ];
    };
    maskableService = "unattended-upgrades.service";
  };

  debian-13 = {
    # x86_64-linux only: the qcow2 extraction path uses libguestfs-with-appliance,
    # whose appliance subpackage is not available on aarch64 in nixpkgs.
    systems = [ "x86_64-linux" ];
    rootfs = makeRootfs.buildRootfs {
      name = "debian-13";
      cloudImgFormat = "qcow2";
      cloudImg = builtins.fetchurl {
        url = "https://cloud.debian.org/images/cloud/trixie/20260413-2447/debian-13-genericcloud-amd64-20260413-2447.qcow2";
        sha256 = "0iclqh20da7mm1ijhks66iqy2dz1shpipia1zwpgxpya1c4gg182";
      };
      excludePatterns = ubuntuExcludePatterns;
      extraDirs = [ "var/lib/apt/lists/partial" ];
    };
    maskableService = "unattended-upgrades.service";
  };
}

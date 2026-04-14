{
  pkgs,
  system ? pkgs.stdenv.hostPlatform.system,
}:
let
  makeRootfs = import ./make-rootfs.nix { inherit pkgs system; };
  images = builtins.fromJSON (builtins.readFile ./images.json);

  fetchCloudImg =
    distroName:
    let
      entry = images.${distroName}.${system} or (throw "Unsupported system for ${distroName}: ${system}");
    in
    builtins.fetchurl {
      inherit (entry) url sha256;
    };

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
    systems = builtins.attrNames images.ubuntu-22_04;
    rootfs = makeRootfs.buildRootfs {
      name = "ubuntu-22_04";
      cloudImg = fetchCloudImg "ubuntu-22_04";
      cloudImgFormat = "tar";
      excludePatterns = ubuntuExcludePatterns;
      extraDirs = [ "var/lib/apt/lists/partial" ];
    };
    maskableService = "unattended-upgrades.service";
  };

  ubuntu-24_04 = {
    systems = builtins.attrNames images.ubuntu-24_04;
    rootfs = makeRootfs.buildRootfs {
      name = "ubuntu-24_04";
      cloudImg = fetchCloudImg "ubuntu-24_04";
      cloudImgFormat = "tar";
      excludePatterns = ubuntuExcludePatterns;
      extraDirs = [ "var/lib/apt/lists/partial" ];
    };
    maskableService = "unattended-upgrades.service";
  };

  debian-13 = {
    systems = builtins.attrNames images.debian-13;
    rootfs = makeRootfs.buildRootfs {
      name = "debian-13";
      cloudImgFormat = "disk-tarball";
      cloudImg = fetchCloudImg "debian-13";
      excludePatterns = ubuntuExcludePatterns;
      extraDirs = [ "var/lib/apt/lists/partial" ];
    };
    maskableService = "unattended-upgrades.service";
  };
}

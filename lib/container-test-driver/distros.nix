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

  commonExcludePatterns = [
    "usr/lib/systemd/system/systemd-remount-fs.service"
    "usr/lib/systemd/system/proc-sys-fs-binfmt_misc.automount"
    "usr/lib/systemd/system/sys-kernel-*"
  ];

  rhelExcludePatterns = commonExcludePatterns ++ [
    "usr/lib/systemd/system/tpm-udev.service"
    "usr/lib/systemd/system/systemd-resolved.service"
    "usr/lib/systemd/system/NetworkManager-wait-online.service"
    "usr/lib/systemd/system/network-online.target.wants/NetworkManager-wait-online.service"
  ];

  rhelRootfsDefaults = {
    excludePatterns = rhelExcludePatterns;
    tarExtraFlags = "--no-selinux";
    extraSetup = ''
      find $out -name '.SELinux*' -delete 2>/dev/null || true
    '';
  };

  firstbootExcludePatterns = [
    "usr/lib/systemd/system/systemd-firstboot.service"
    "usr/lib/systemd/system/sysinit.target.wants/systemd-firstboot.service"
    "usr/lib/systemd/system/first-boot-complete.target"
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

  debian-12 = {
    systems = [
      "x86_64-linux"
      "aarch64-linux"
    ];
    rootfs = makeRootfs.buildRootfs {
      name = "debian-12";
      cloudImg =
        if system == "x86_64-linux" then
          builtins.fetchurl {
            url = "https://images.linuxcontainers.org/images/debian/bookworm/amd64/cloud/20260410_05:24/rootfs.tar.xz";
            sha256 = "0qkdlpa0pggyzz2y1akzndic2ksp5sw1b96v3494q0k5awh1f67n";
          }
        else if system == "aarch64-linux" then
          builtins.fetchurl {
            url = "https://images.linuxcontainers.org/images/debian/bookworm/arm64/cloud/20260410_05:24/rootfs.tar.xz";
            sha256 = "1bz5y90y4a0n3rh4lwmjc04v39k4igkibgkz1h0cs2lsj72zm0gz";
          }
        else
          throw "Unsupported system: ${system}";
      excludePatterns = rhelExcludePatterns ++ firstbootExcludePatterns;
      extraDirs = [ "var/lib/apt/lists/partial" ];
      extraSetup = ''
        ln -sf /usr/lib/systemd/system/multi-user.target $out/etc/systemd/system/default.target
        echo 'en_US.UTF-8 UTF-8' > $out/etc/locale.gen
        echo 'LANG=en_US.UTF-8' > $out/etc/locale.conf
      '';
    };
    maskableService = "cron.service";
  };

  fedora-42 = {
    systems = [
      "x86_64-linux"
      "aarch64-linux"
    ];
    rootfs = makeRootfs.buildRootfs (
      rhelRootfsDefaults
      // {
        name = "fedora-42";
        cloudImg =
          if system == "x86_64-linux" then
            builtins.fetchurl {
              url = "https://images.linuxcontainers.org/images/fedora/42/amd64/cloud/20260409_20:33/rootfs.tar.xz";
              sha256 = "0ys3l999xa2s8zpvr95x23ff5786g4zfvh26fqskh2g0k17c0j5m";
            }
          else if system == "aarch64-linux" then
            builtins.fetchurl {
              url = "https://images.linuxcontainers.org/images/fedora/42/arm64/cloud/20260409_20:33/rootfs.tar.xz";
              sha256 = "0rwyi2mxg1vz8zixqjcj18xa38275wbwp5dpi566gkx4d52j5673";
            }
          else
            throw "Unsupported system: ${system}";
        extraDirs = [ "var/cache/dnf" ];
      }
    );
    maskableService = "crond.service";
  };

  rocky-9 = {
    systems = [
      "x86_64-linux"
      "aarch64-linux"
    ];
    rootfs = makeRootfs.buildRootfs (
      rhelRootfsDefaults
      // {
        name = "rocky-9";
        cloudImg =
          if system == "x86_64-linux" then
            builtins.fetchurl {
              url = "https://images.linuxcontainers.org/images/rockylinux/9/amd64/cloud/20260410_02:06/rootfs.tar.xz";
              sha256 = "0jwkc5zn0x39s1nlfp0950gpy95gzvfxfnn6p6xphn1plr2zx4c7";
            }
          else if system == "aarch64-linux" then
            builtins.fetchurl {
              url = "https://images.linuxcontainers.org/images/rockylinux/9/arm64/cloud/20260410_02:06/rootfs.tar.xz";
              sha256 = "12w3jf0q8wwcsqlw0drfz1yhlhcid5avanh01zqlm17allh7biif";
            }
          else
            throw "Unsupported system: ${system}";
      }
    );
    maskableService = "crond.service";
  };

  almalinux-9 = {
    systems = [
      "x86_64-linux"
      "aarch64-linux"
    ];
    rootfs = makeRootfs.buildRootfs (
      rhelRootfsDefaults
      // {
        name = "almalinux-9";
        cloudImg =
          if system == "x86_64-linux" then
            builtins.fetchurl {
              url = "https://images.linuxcontainers.org/images/almalinux/9/amd64/cloud/20260409_23:08/rootfs.tar.xz";
              sha256 = "0ssrmqx3wlr0nbxs9r85wmc92g52l477164y1rhrl2pxy9h130zr";
            }
          else if system == "aarch64-linux" then
            builtins.fetchurl {
              url = "https://images.linuxcontainers.org/images/almalinux/9/arm64/cloud/20260409_23:08/rootfs.tar.xz";
              sha256 = "1fr7wgapqqyl3i0d0nqiwz2q08bij9l3smyw06xzhk6421fvwdf3";
            }
          else
            throw "Unsupported system: ${system}";
      }
    );
    maskableService = "crond.service";
  };

  archlinux = {
    systems = [ "x86_64-linux" ];
    rootfs = makeRootfs.buildRootfs {
      name = "archlinux";
      cloudImg =
        if system == "x86_64-linux" then
          builtins.fetchurl {
            url = "https://images.linuxcontainers.org/images/archlinux/current/amd64/cloud/20260410_04:18/rootfs.tar.xz";
            sha256 = "0g3xjqdhf3ycqs2gxmbga3d1wvbiqy5s51s3wp7l2jysbyjlywkv";
          }
        else
          throw "Arch Linux container images are only available for x86_64-linux";
      excludePatterns = commonExcludePatterns ++ firstbootExcludePatterns;
      extraDirs = [ "var/lib/pacman" ];
      extraSetup = ''
        echo 'en_US.UTF-8 UTF-8' > $out/etc/locale.gen
        echo 'LANG=en_US.UTF-8' > $out/etc/locale.conf
        ln -sf /usr/share/zoneinfo/UTC $out/etc/localtime
        echo 'archlinux' > $out/etc/hostname
      '';
    };
    maskableService = "systemd-timesyncd.service";
  };
}

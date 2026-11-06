{
  pkgs,
  system ? pkgs.stdenv.hostPlatform.system,
}:
let
  nixArtifacts = import ./nix-artifacts.nix { inherit system; };
in
{
  buildRootfs =
    {
      name,
      cloudImg,
      cloudImgFormat,
      excludePatterns ? [ ],
      extraDirs ? [ ],
      extraSetup ? "",
      tarExtraFlags ? "",
      tarCompression ? "-J",
    }@rootFsConfig:
    let
      excludeArgs = builtins.concatStringsSep " \\\n        " (
        map (p: "--exclude='${p}'") excludePatterns
      );
      mkdirCommands = builtins.concatStringsSep "\n    " (map (d: "mkdir -p $out/${d}") extraDirs);
      excludePruneCommands = builtins.concatStringsSep "\n    " (
        map (p: "rm -rf $out/${p}") excludePatterns
      );
      extractDiskImage = toRawImage: ''
        set -euo pipefail

        workdir=$(mktemp -d)
        ${toRawImage}

        # Pick the largest partition
        read -r start size <<<"$(sfdisk -J "$rawimg" \
          | jq -r '.partitiontable.partitions | max_by(.size) | "\(.start) \(.size)"')"
        dd if="$rawimg" of="$workdir/root.img" \
           bs=512 skip="$start" count="$size" conv=sparse status=none
        rm -f "$rawimg"

        fstype=$(blkid -o value -s TYPE "$workdir/root.img")
        case "$fstype" in
          ext4)
            debugfs -R "rdump / $out" "$workdir/root.img" >/dev/null 2>&1
            ;;
          btrfs)
            mkdir "$workdir/fs"
            btrfs restore -s -m -S "$workdir/root.img" "$workdir/fs" >/dev/null
            fstab=$(ls "$workdir"/fs/*/etc/fstab | head -n1)
            awk '$3 == "btrfs" && match($4, /subvol=[^,]*/) {
                   subvol = substr($4, RSTART + 7, RLENGTH - 7)
                   sub(/^\//, "", subvol)
                   print $2, subvol
                 }' "$fstab" | sort > "$workdir/subvols"
            if [ "$(head -n1 "$workdir/subvols" | cut -d' ' -f1)" != / ]; then
              echo "btrfs: no subvolume mounted on / in $fstab" >&2
              exit 1
            fi
            while read -r mnt subvol; do
              rm -rf "$out$mnt"
              mv "$workdir/fs/$subvol" "$out$mnt"
            done < "$workdir/subvols"
            ;;
          *)
            echo "unsupported root filesystem: $fstype" >&2
            exit 1
            ;;
        esac

        # debugfs rdump and btrfs restore have no --exclude, so apply
        # excludePatterns via a post-extraction prune pass. Also strip /dev/*
        # to match the tar path (which uses tar --exclude='dev/*').
        rm -rf $out/dev/*
        ${excludePruneCommands}

        rm -rf "$workdir"
      '';
      isDiskImage = cloudImgFormat == "disk-tarball" || cloudImgFormat == "disk-qcow2";
      extractCommand =
        if cloudImgFormat == "tar" then
          ''
            tar --exclude='dev/*' \
                ${excludeArgs} \
                ${tarExtraFlags} \
                ${tarCompression}xf ${cloudImg} -C $out
          ''
        else if cloudImgFormat == "qcow2" then
          ''
            LIBGUESTFS_BACKEND=direct \
              guestfish --ro -a ${cloudImg} -i tar-out / - \
              | tar --exclude='dev/*' \
                    ${excludeArgs} \
                    -C $out -x
          ''
        else if cloudImgFormat == "disk-tarball" then
          extractDiskImage ''
            tar -C "$workdir" -xf ${cloudImg}
            rawimg=$(ls "$workdir"/*.raw | head -n1)
            if [ -z "$rawimg" ]; then
              echo "disk-tarball: no *.raw file inside ${cloudImg}" >&2
              exit 1
            fi
          ''
        else if cloudImgFormat == "disk-qcow2" then
          extractDiskImage ''
            rawimg="$workdir/disk.raw"
            qemu-img convert -O raw ${cloudImg} "$rawimg"
          ''
        else
          throw "buildRootfs: unsupported cloudImgFormat '${cloudImgFormat}' (expected 'tar', 'qcow2', 'disk-tarball', or 'disk-qcow2')";
      nativeBuildInputs = [
        pkgs.xz
      ]
      ++ pkgs.lib.optionals (cloudImgFormat == "qcow2") [ pkgs.libguestfs-with-appliance ]
      ++ pkgs.lib.optionals isDiskImage [
        pkgs.util-linux
        pkgs.e2fsprogs
        pkgs.btrfs-progs
        pkgs.fakeroot
        pkgs.jq
      ]
      ++ pkgs.lib.optionals (cloudImgFormat == "disk-qcow2") [ pkgs.qemu-utils ];
      buildCommand = ''
        tarball=$out
        out=$PWD/rootfs
        mkdir -p $out

        # Extract cloud image, excluding container-incompatible services
        ${extractCommand}

        # Ensure build user can modify all extracted files
        chmod -R u+rwX $out

        # Ensure FHS compatibility symlinks exist (merged-usr layout).
        # Some distros already have these as symlinks; others have real directories.
        for dir in bin lib lib64 sbin; do
          if [ -L "$out/$dir" ]; then
            # Already a symlink, replace to ensure correct target
            rm -f "$out/$dir"
            ln -sf "usr/$dir" "$out/$dir"
          elif [ -d "$out/$dir" ] && [ -d "$out/usr/$dir" ]; then
            # Real directory alongside usr/ counterpart: merge contents into usr/ and symlink
            cp -a "$out/$dir/." "$out/usr/$dir/" 2>/dev/null || true
            rm -rf "$out/$dir"
            ln -sf "usr/$dir" "$out/$dir"
          elif [ ! -e "$out/$dir" ]; then
            # Doesn't exist at all, create symlink if usr/ counterpart exists
            [ -d "$out/usr/$dir" ] && ln -sf "usr/$dir" "$out/$dir"
          fi
        done

        # Container marker for systemd
        mkdir -p $out/run/systemd
        echo 'systemd-nspawn' > $out/run/systemd/container

        # Include nix-installer binary
        mkdir -p $out/usr/local/bin
        install -m755 ${nixArtifacts.nix-installer} $out/usr/local/bin/nix-installer

        # Create marker to indicate Nix needs installation
        touch $out/.nix-not-installed

        # Create distro-specific directories
        ${mkdirCommands}

        # Run distro-specific setup
        ${extraSetup}

        tar -C $out --sparse${pkgs.lib.optionalString isDiskImage " --mode=ug-s"} -cf $tarball .
      '';
    in
    pkgs.runCommand "rootfs-${name}.tar"
      {
        inherit nativeBuildInputs;
        passthru = {
          inherit rootFsConfig;
        };
      }
      # The sandbox rejects chown and setuid chmod, so dumped files lose their
      # modes. fakeroot keeps them for the final tar; setuid/setgid are
      # dropped there because the driver extracts in the sandbox too.
      (
        if isDiskImage then
          "fakeroot ${pkgs.writeShellScript "build-rootfs-${name}" "set -euo pipefail\n${buildCommand}"}"
        else
          buildCommand
      );
}

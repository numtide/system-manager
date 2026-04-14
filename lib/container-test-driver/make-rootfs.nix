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
    }:
    let
      excludeArgs = builtins.concatStringsSep " \\\n        " (
        map (p: "--exclude='${p}'") excludePatterns
      );
      mkdirCommands = builtins.concatStringsSep "\n    " (map (d: "mkdir -p $out/${d}") extraDirs);
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
        else
          throw "buildRootfs: unsupported cloudImgFormat '${cloudImgFormat}' (expected 'tar' or 'qcow2')";
      nativeBuildInputs = [
        pkgs.xz
      ]
      ++ pkgs.lib.optionals (cloudImgFormat == "qcow2") [ pkgs.libguestfs-with-appliance ];
    in
    pkgs.runCommand "rootfs-${name}"
      {
        inherit nativeBuildInputs;
      }
      ''
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
      '';
}

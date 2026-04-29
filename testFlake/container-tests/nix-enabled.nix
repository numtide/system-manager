{ forEachDistro, ... }:

forEachDistro "nix-enabled" {
  modules = [
    (
      { ... }:
      {
        nix.enable = true;
      }
    )
  ];
  testScriptFunction =
    { toplevel, hostPkgs, ... }:
    ''
      start_all()

      machine.wait_for_unit("multi-user.target")

      with subtest("Pre-existing nix.conf before activation"):
          assert machine.file("/etc/nix/nix.conf").exists, "/etc/nix/nix.conf should exist before activation"
          original_nix_conf = machine.succeed("cat /etc/nix/nix.conf")

      machine.activate()
      machine.wait_for_unit("system-manager.target")

      with subtest("nix.conf is managed after activation"):
          nix_conf = machine.file("/etc/nix/nix.conf")
          assert nix_conf.exists, "/etc/nix/nix.conf should exist"
          assert nix_conf.contains("experimental-features"), "nix.conf should contain experimental-features"
          assert nix_conf.contains("nix-command"), "nix.conf should contain nix-command"
          assert nix_conf.contains("flakes"), "nix.conf should contain flakes"

      with subtest("Re-activation succeeds"):
          machine.activate()
          machine.wait_for_unit("system-manager.target")
          nix_conf = machine.file("/etc/nix/nix.conf")
          assert nix_conf.exists, "/etc/nix/nix.conf should still exist after re-activation"
          assert nix_conf.contains("flakes"), "nix.conf should still contain flakes"

      with subtest("/root/.nix-channels is created with the default channel"):
          channels_file = machine.file("/root/.nix-channels")
          assert channels_file.exists, "/root/.nix-channels should exist"
          channels_content = channels_file.content_string
          assert "channels.nixos.org/nixos-unstable" in channels_content, (
              f"Expected nixos-unstable channel, got: {channels_content!r}"
          )
          assert "nixos" in channels_content, (
              f"Expected nixos channel name, got: {channels_content!r}"
          )

      with subtest("NIX_PATH is exported in login shell"):
          nix_path = machine.succeed("bash --login -c 'echo $NIX_PATH'").strip()
          assert "nixpkgs=" in nix_path, f"Expected nixpkgs= entry, got: {nix_path!r}"
          assert "/nix/var/nix/profiles/per-user/root/channels" in nix_path, (
              f"Expected per-user root channels path, got: {nix_path!r}"
          )

      with subtest("nix-channel binary is available on PATH"):
          machine.succeed("test -e /run/system-manager/sw/bin/nix-channel")

      with subtest("Deactivation restores original nix.conf"):
          machine.succeed("${toplevel}/bin/deactivate")
          restored_nix_conf = machine.succeed("cat /etc/nix/nix.conf")
          assert restored_nix_conf == original_nix_conf, f"nix.conf content differs after deactivation:\n  original: {original_nix_conf!r}\n  restored: {restored_nix_conf!r}"
    '';
}

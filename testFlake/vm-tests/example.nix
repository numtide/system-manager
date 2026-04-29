{
  forEachImage,
  newConfig,
  system-manager,
  ...
}:

forEachImage "example" {
  modules = [
    ../../examples/example.nix
  ];
  extraPathsToRegister = [
    newConfig
    ../sops/age-keys.txt
  ];
  testScriptFunction =
    { toplevel, hostPkgs, ... }:
    #python
    ''
      # Start all machines in parallel
      start_all()

      vm.wait_for_unit("default.target")
      vm.succeed("cp ${../sops/age-keys.txt} /run/age-keys.txt")

      # Capture original shell paths before activation (for deactivation restoration check)
      root_shell_before = vm.succeed("getent passwd root").strip().split(":")[-1]
      nobody_shell_before = vm.succeed("getent passwd nobody").strip().split(":")[-1]

      vm.succeed("touch /etc/foo_test")
      output = vm.fail("${toplevel}/bin/activate 2>&1")
      assert "Unmanaged path already exists" in output, f"Expected unmanaged path error, got: {output}"
      vm.succeed("rm /etc/foo_test")

      ${system-manager.lib.activateProfileSnippet {
        node = "vm";
        profile = toplevel;
      }}
      vm.wait_for_unit("system-manager.target")

      vm.succeed("systemctl status service-9.service")
      vm.succeed("test -f /etc/baz/bar/foo2")
      vm.succeed("test -f /etc/a/nested/example/foo3")
      vm.succeed("test -f /etc/foo.conf")
      vm.succeed("grep -F 'launch_the_rockets = true' /etc/foo.conf")
      vm.fail("grep -F 'launch_the_rockets = false' /etc/foo.conf")

      uid = vm.succeed("stat -c %u /etc/with_ownership").strip()
      gid = vm.succeed("stat -c %g /etc/with_ownership").strip()
      assert uid == "5", f"uid was {uid}, expected 5"
      assert gid == "6", f"gid was {gid}, expected 6"

      vm.succeed("useradd luj")
      vm.succeed("echo \"luj:test\" | chpasswd")

      print(vm.succeed("cat /etc/passwd"))
      passwd_out = vm.succeed("passwd -S luj | awk '{print $2}'")
      assert "P" in passwd_out, f"Expected luj to be unlocked with 'P' status, got: {passwd_out}"

      user = vm.succeed("stat -c %U /etc/with_ownership2").strip()
      group = vm.succeed("stat -c %G /etc/with_ownership2").strip()
      assert user == "nobody", f"user was {user}, expected nobody"
      assert group == "users", f"group was {group}, expected users"

      vm.fail("test -e /etc/with_ownership.uid")
      vm.fail("test -e /etc/with_ownership.gid")
      vm.fail("test -e /etc/with_ownership.mode")
      vm.fail("test -e /etc/with_ownership2.uid")
      vm.fail("test -e /etc/with_ownership2.gid")
      vm.fail("test -e /etc/with_ownership2.mode")

      vm.succeed("test -d /var/tmp/system-manager")
      vm.succeed("test -d /var/tmp/sample")

      vm.succeed("test -f /etc/tmpfiles.d/sample.conf")
      vm.succeed("test -f /etc/tmpfiles.d/00-system-manager.conf")

      ${system-manager.lib.activateProfileSnippet {
        node = "vm";
        profile = newConfig;
      }}

      print(vm.succeed("cat /run/secrets/test"))

      vm.succeed("systemctl status new-service.service")
      vm.fail("systemctl status service-9.service")
      vm.fail("test -f /etc/a/nested/example/foo3")
      vm.fail("test -f /etc/baz/bar/foo2")
      vm.fail("test -f /etc/systemd/system/nginx.service")
      vm.succeed("test -f /etc/foo_new")

      vm.succeed("test -d /var/tmp/system-manager")
      vm.succeed("touch /var/tmp/system-manager/foo1")

      # Simulate a reboot, to check that the services defined with
      # system-manager start correctly after a reboot.
      # TODO: can we find an easy way to really reboot the VM and not
      # loose the root FS state?
      vm.systemctl("isolate rescue.target")
      # We need to send a return character to dismiss the rescue-mode prompt
      vm.send_key("ret")
      vm.systemctl("isolate default.target")
      vm.wait_for_unit("default.target")

      vm.succeed("systemctl status new-service.service")
      vm.fail("systemctl status service-9.service")
      vm.fail("test -f /etc/a/nested/example/foo3")
      vm.fail("test -f /etc/baz/bar/foo2")
      vm.succeed("test -f /etc/foo_new")

      vm.succeed("id -u zimbatm")

      print(vm.succeed("systemctl status userborn.service"))
      print(vm.succeed("journalctl -u userborn.service"))
      print(vm.succeed("cat /var/lib/userborn/previous-userborn.json"))

      print(vm.succeed("cat /etc/passwd"))
      passwd_out = vm.succeed("passwd -S luj | awk '{print $2}'")
      assert "P" in passwd_out, f"Expected luj to be unlocked with 'P' status, got: {passwd_out}"

      nix_trusted_users = vm.succeed("${hostPkgs.nix}/bin/nix config show trusted-users").strip()
      assert "zimbatm" in nix_trusted_users, f"Expected 'zimbatm' to be in trusted-users, got {nix_trusted_users}"

      luj_entry = vm.succeed("getent passwd luj").strip()
      assert luj_entry != "", "Expected user 'luj' to exist"

      # Verify zimbatm user exists with correct shell path
      zimbatm_entry = vm.succeed("getent passwd zimbatm").strip()
      assert "/run/system-manager/sw/bin/bash" in zimbatm_entry, f"Expected shell to be /run/system-manager/sw/bin/bash, got: {zimbatm_entry}"

      # Verify root and nobody shells were rewritten to system-manager paths during activation
      root_entry_active = vm.succeed("getent passwd root").strip()
      assert "/run/system-manager/sw" in root_entry_active, f"Expected root shell under /run/system-manager/sw, got: {root_entry_active}"
      nobody_entry_active = vm.succeed("getent passwd nobody").strip()
      assert "/run/system-manager/sw" in nobody_entry_active, f"Expected nobody shell under /run/system-manager/sw, got: {nobody_entry_active}"

      # Verify wheel group exists and zimbatm is in it
      wheel_entry = vm.succeed("getent group wheel").strip()
      print(f"Wheel group: {wheel_entry}")
      assert wheel_entry != "", "Expected wheel group to exist"

      zimbatm_groups = vm.succeed("id -Gn zimbatm").strip()
      print(f"zimbatm groups: {zimbatm_groups}")
      assert "wheel" in zimbatm_groups, f"Expected zimbatm to be in wheel group, got: {zimbatm_groups}"

      # Verify zimbatm is added to pre-existing sudo group (gid 27 on Ubuntu)
      # This tests that userborn correctly adds users to groups that existed
      # on the system before system-manager was activated.
      sudo_entry = vm.succeed("getent group sudo").strip()
      print(f"Sudo group: {sudo_entry}")
      assert "zimbatm" in sudo_entry, f"Expected zimbatm in sudo group, got: {sudo_entry}"
      assert "sudo" in zimbatm_groups, f"Expected zimbatm to be in sudo group, got: {zimbatm_groups}"

      # Verify /etc/shadow has correct permissions after userborn activation
      shadow_mode = vm.succeed("stat -c '%a' /etc/shadow").strip()
      shadow_group = vm.succeed("stat -c '%G' /etc/shadow").strip()
      print(f"Shadow permissions: mode={shadow_mode}, group={shadow_group}")
      assert shadow_mode == "640", f"Expected /etc/shadow mode 640, got: {shadow_mode}"
      assert shadow_group == "shadow", f"Expected /etc/shadow group shadow, got: {shadow_group}"

      zimbatm_shadow_before = vm.succeed("grep '^zimbatm:' /etc/shadow").strip()
      print(f"Shadow entry before deactivation: {zimbatm_shadow_before}")
      assert not zimbatm_shadow_before.startswith("zimbatm:!*"), f"Expected unlocked account before deactivation, got: {zimbatm_shadow_before}"

      # Re-activate the same profile to verify idempotency and no ERROR in output
      ${system-manager.lib.activateProfileSnippet {
        node = "vm";
        profile = newConfig;
      }}
      vm.succeed("systemctl status new-service.service")
      vm.succeed("test -f /etc/foo_new")

      ${system-manager.lib.deactivateProfileSnippet {
        node = "vm";
        profile = newConfig;
      }}
      vm.fail("systemctl status new-service.service")
      vm.fail("test -f /etc/foo_new")

      # userborn never deletes users
      zimbatm_entry = vm.succeed("getent passwd zimbatm").strip()
      assert zimbatm_entry != "", f"Expected user 'zimbatm' to persist after deactivation, got empty"

      # userborn locks user in shadow (password = "!*") after deactivation
      zimbatm_shadow = vm.succeed("grep '^zimbatm:' /etc/shadow").strip()
      print(f"Shadow entry after deactivation: {zimbatm_shadow}")
      assert zimbatm_shadow.startswith("zimbatm:!*"), f"Expected locked account (zimbatm:!*), got: {zimbatm_shadow}"

      # Stateful user 'luj' (not managed by userborn) should NOT be locked
      luj_shadow = vm.succeed("grep '^luj:' /etc/shadow").strip()
      print(f"Stateful user shadow after deactivation: {luj_shadow}")
      assert not luj_shadow.startswith("luj:!*"), f"Stateful user 'luj' should NOT be locked after deactivation, got: {luj_shadow}"

      # Verify /etc/shadow permissions are preserved after deactivation
      shadow_mode_after = vm.succeed("stat -c '%a' /etc/shadow").strip()
      shadow_group_after = vm.succeed("stat -c '%G' /etc/shadow").strip()
      print(f"Shadow permissions after deactivation: mode={shadow_mode_after}, group={shadow_group_after}")
      assert shadow_mode_after == "640", f"Expected /etc/shadow mode 640 after deactivation, got: {shadow_mode_after}"
      assert shadow_group_after == "shadow", f"Expected /etc/shadow group shadow after deactivation, got: {shadow_group_after}"

      # Verify /etc/passwd shells are restored to original values after deactivation
      root_shell_after = vm.succeed("getent passwd root").strip().split(":")[-1]
      assert root_shell_after == root_shell_before, f"Expected root shell restored to {root_shell_before}, got: {root_shell_after}"

      nobody_shell_after = vm.succeed("getent passwd nobody").strip().split(":")[-1]
      assert nobody_shell_after == nobody_shell_before, f"Expected nobody shell restored to {nobody_shell_before}, got: {nobody_shell_after}"

      # Managed user shell should not point to /run/system-manager/sw after deactivation
      zimbatm_shell_after = vm.succeed("getent passwd zimbatm").strip().split(":")[-1]
      assert "/run/system-manager/sw" not in zimbatm_shell_after, f"Expected zimbatm shell to not reference system-manager after deactivation, got: {zimbatm_shell_after}"
    '';
}

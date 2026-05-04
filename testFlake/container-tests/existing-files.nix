# Test that pre-existing files are backed up and restored when replaceExisting
# is enabled, and that systemd .wants/.requires symlinks are auto-replaced
# with backup on systems where those entries already exist.
{
  forEachDistro,
  ...
}:

forEachDistro "existing-files" {
  modules = [
    (
      { lib, pkgs, ... }:
      {
        config = {
          environment.etc = {
            "force-copy-test" = {
              text = "managed copy content\n";
              mode = "0644";
              replaceExisting = true;
            };
            "force-symlink-test" = {
              text = "managed symlink content\n";
              replaceExisting = true;
            };
            "no-replace-test" = {
              text = "should not appear\n";
              mode = "0644";
            };
          };

          systemd.timers.existing = {
            wantedBy = [ "timers.target" ];
            timerConfig = {
              OnCalendar = "daily";
              Persistent = true;
            };
          };
          systemd.services.existing = {
            serviceConfig.Type = "oneshot";
            wantedBy = [ "system-manager.target" ];
            script = "true";
          };
        };
      }
    )
  ];
  extraPathsToRegister = [ ];
  testScriptFunction =
    { toplevel, hostPkgs, ... }:
    ''
      start_all()
      machine.wait_for_unit("multi-user.target")

      # Create pre-existing files that system-manager will replace
      machine.succeed("echo -n 'original copy content' > /etc/force-copy-test")
      machine.succeed("echo -n 'original symlink content' > /etc/force-symlink-test")
      machine.succeed("echo -n 'do not touch' > /etc/no-replace-test")

      # Create pre-existing .wants symlink (simulating Ubuntu's pre-installed timers)
      machine.succeed("mkdir -p /etc/systemd/system/timers.target.wants")
      machine.succeed("ln -sf /lib/systemd/system/fake-existing.timer /etc/systemd/system/timers.target.wants/existing.timer")

      # Activate directly because the no-replace-test entry produces an
      # expected error that causes a non-zero exit code.
      output = machine.fail("${toplevel}/bin/activate 2>&1")
      assert "File /etc/no-replace-test already exists" in output, f"Expected no-replace error, got: {output}"
      no_replace = machine.succeed("cat /etc/no-replace-test").strip()
      assert no_replace == "do not touch", f"Expected untouched file, got: {no_replace}"
      machine.fail("test -e /etc/no-replace-test.system-manager-backup")

      managed_copy = machine.succeed("cat /etc/force-copy-test").strip()
      assert "managed copy content" in managed_copy, f"Expected managed copy content, got: {managed_copy}"
      backup_copy = machine.succeed("cat /etc/force-copy-test.system-manager-backup").strip()
      assert backup_copy == "original copy content", f"Expected original backup, got: {backup_copy}"

      machine.succeed("test -L /etc/force-symlink-test")
      managed_symlink = machine.succeed("cat /etc/force-symlink-test").strip()
      assert "managed symlink content" in managed_symlink, f"Expected managed symlink content, got: {managed_symlink}"
      backup_symlink = machine.succeed("cat /etc/force-symlink-test.system-manager-backup").strip()
      assert backup_symlink == "original symlink content", f"Expected original symlink backup, got: {backup_symlink}"

      # Verify the timer is pulled in via system-manager.target
      machine.succeed("test -L /etc/systemd/system/system-manager.target.wants/existing.timer")

      # Verify the pre-existing timers.target.wants symlink is left untouched
      existing_wants = machine.succeed("readlink /etc/systemd/system/timers.target.wants/existing.timer").strip()
      assert "fake-existing.timer" in existing_wants, f"Expected pre-existing .wants symlink untouched, got: {existing_wants}"

      # Verify the timer unit content matches the declared config
      timer_content = machine.succeed("cat /etc/systemd/system/existing.timer")
      assert "OnCalendar=daily" in timer_content, f"Expected OnCalendar=daily in timer unit, got: {timer_content}"
      assert "Persistent=true" in timer_content, f"Expected Persistent=true in timer unit, got: {timer_content}"

      # Deactivate and verify backups are restored
      machine.succeed("${toplevel}/bin/deactivate 2>&1 | tee /tmp/output.log")
      machine.succeed("! grep -F 'ERROR' /tmp/output.log")

      # Verify originals restored from backups
      restored_copy = machine.succeed("cat /etc/force-copy-test").strip()
      assert restored_copy == "original copy content", f"Expected restored original, got: {restored_copy}"
      machine.fail("test -e /etc/force-copy-test.system-manager-backup")

      restored_symlink = machine.succeed("cat /etc/force-symlink-test").strip()
      assert restored_symlink == "original symlink content", f"Expected restored original, got: {restored_symlink}"
      machine.fail("test -e /etc/force-symlink-test.system-manager-backup")

      # Pre-existing timers.target.wants symlink should still be present (never touched)
      restored_wants = machine.succeed("readlink /etc/systemd/system/timers.target.wants/existing.timer").strip()
      assert "fake-existing.timer" in restored_wants, f"Expected pre-existing .wants symlink, got: {restored_wants}"

      # Verify no-replace-test was never touched
      no_replace_after = machine.succeed("cat /etc/no-replace-test").strip()
      assert no_replace_after == "do not touch", f"Expected untouched file after deactivation, got: {no_replace_after}"
    '';
}

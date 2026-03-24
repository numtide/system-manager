{ forEachDistro, ... }:

forEachDistro "empty-config" {
  modules = [ ];
  testScriptFunction =
    { toplevel, hostPkgs, ... }:
    ''
      import json

      start_all()

      machine.wait_for_unit("multi-user.target")

      with subtest("etcFiles.json contains only system-manager infrastructure"):
          etc_json = json.loads(machine.succeed("cat ${toplevel}/etcFiles/etcFiles.json"))
          etc_entries = set(etc_json["entries"].keys())
          allowed_entries = {
              "profile.d/system-manager-path.sh",
              "systemd/system",
              "tmpfiles.d",
          }
          unexpected_etc = etc_entries - allowed_entries
          assert not unexpected_etc, \
              f"Empty config should not produce etc entries beyond infrastructure: {unexpected_etc}"

      with subtest("services.json contains only system-manager infrastructure"):
          svc_json = json.loads(machine.succeed("cat ${toplevel}/services/services.json"))
          service_names = set(svc_json.keys())
          allowed_services = {
              "system-manager.target",
              "sysinit-reactivation.target",
              "system-manager-path.service",
              "userborn.service",
              "suid-sgid-wrappers.service",
              "run-wrappers.mount",
          }
          unexpected_svc = service_names - allowed_services
          assert not unexpected_svc, \
              f"Empty config should not produce services beyond infrastructure: {unexpected_svc}"
          # No service should be masked
          for name, config in svc_json.items():
              assert not config.get("masked", False), \
                  f"Infrastructure service {name} should not be masked"

      with subtest("No etc entries replace existing files"):
          etc_json = json.loads(machine.succeed("cat ${toplevel}/etcFiles/etcFiles.json"))
          for name, entry in etc_json["entries"].items():
              assert not entry.get("replaceExisting", False), \
                  f"etc entry {name} should not replace existing files in empty config"

      # Exhaustive list of paths the empty config is allowed to add or modify.
      # update this list only after confirming the change is intentional.
      allowed_changes = {
          "profile.d/system-manager-path.sh",
          # systemd unit files
          "systemd/system/system-manager.target",
          "systemd/system/sysinit-reactivation.target",
          "systemd/system/system-manager-path.service",
          "systemd/system/userborn.service",
          "systemd/system/suid-sgid-wrappers.service",
          "systemd/system/run-wrappers.mount",
          "systemd/system/default.target.wants/system-manager.target",
          "systemd/system/system-manager.target.wants/suid-sgid-wrappers.service",
          "systemd/system/system-manager.target.wants/system-manager-path.service",
          "systemd/system/sysinit-reactivation.target.requires/userborn.service",
          "systemd/system/sysinit.target.wants/userborn.service",
          # tmpfiles
          "tmpfiles.d/00-system-manager.conf",
          "tmpfiles.d/home-directories.conf",
          # userborn always touches passwd/group/shadow even with empty config
          "passwd",
          "group",
          "shadow",
          # mtab points to /proc/self/mounts, content changes with run-wrappers.mount
          "mtab",
          # resolv.conf is a dangling symlink in containers
          "resolv.conf",
      }

      def is_expected(path: str) -> bool:
          return path in allowed_changes

      def snapshot_etc() -> dict[str, str]:
          # sha256sum output: "hash  /etc/path"
          output = machine.succeed("find -L /etc -not -path '/etc/.system-manager-static/*' -not -type d -exec sha256sum {} + 2>/dev/null || true")
          snapshot: dict[str, str] = {}
          for line in output.strip().split("\n"):
              if not line:
                  continue
              sha, filepath = line.split("  ", 1)
              rel = filepath.removeprefix("/etc/")
              snapshot[rel] = sha
          return snapshot

      with subtest("Snapshot /etc before activation"):
          before = snapshot_etc()

      activation_logs = machine.activate()
      with subtest("Activation produces no errors"):
          for line in activation_logs.split("\n"):
              assert "ERROR" not in line, f"Unexpected error in activation: {line}"

      machine.wait_for_unit("system-manager.target")

      with subtest("Static environment symlink exists"):
          assert machine.file("/etc/.system-manager-static").is_symlink, \
              "/etc/.system-manager-static should be a symlink after activation"

      with subtest("No unexpected changes to /etc after activation"):
          after = snapshot_etc()
          added = [p for p in (set(after) - set(before)) if not is_expected(p)]
          removed = [p for p in (set(before) - set(after)) if not is_expected(p)]
          changed = [p for p in set(before) & set(after) if before[p] != after[p] and not is_expected(p)]
          assert not added, f"Unexpected new files in /etc: {added}"
          assert not removed, f"Files unexpectedly removed from /etc: {removed}"
          assert not changed, f"Unexpected modified files in /etc: {changed}"

      with subtest("Deactivation restores original state exactly"):
          machine.succeed("${toplevel}/bin/deactivate")
          restored = snapshot_etc()
          # userborn changes to passwd/group/shadow are not reversible
          userborn_files = {"passwd", "passwd-", "group", "group-", "shadow", "shadow-"}
          added = [p for p in (set(restored) - set(before)) if p not in userborn_files]
          removed = [p for p in (set(before) - set(restored)) if p not in userborn_files]
          changed = [p for p in set(before) & set(restored) if before[p] != restored[p] and p not in userborn_files]
          assert not added, f"Deactivation left new files: {added}"
          assert not removed, f"Deactivation removed files: {removed}"
          assert not changed, f"Deactivation left modified files: {changed}"
    '';
}

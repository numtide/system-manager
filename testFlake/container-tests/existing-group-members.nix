# Test that userborn preserves existing group members when activating and deactivating.
{
  forEachDistro,
  system-manager,
  ...
}:

forEachDistro "existing-group-members" {
  modules = [
    (
      { lib, pkgs, ... }:
      {
        config = {
          services.userborn.enable = true;

          # Declare a user that will be added to wheel
          users.users.manageduser = {
            isNormalUser = true;
            extraGroups = [ "wheel" ];
            initialPassword = "test123";
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

      machine.succeed("useradd -m zimbatm")

      machine.succeed("groupadd -f wheel")
      machine.succeed("usermod -aG wheel zimbatm")

      wheel_members_before = machine.succeed("getent group wheel").strip()
      print(f"Wheel group before activation: {wheel_members_before}")
      assert "zimbatm" in wheel_members_before, "zimbatm should be in wheel group before activation"

      # Activate system-manager with userborn
      machine.activate()

      # Wait for userborn to complete
      machine.wait_for_unit("system-manager.target")
      print(machine.succeed("systemctl status userborn.service"))
      print(machine.succeed("journalctl -u userborn.service"))

      # Verify the managed user was created and is in wheel
      machine.succeed("id -u manageduser")
      managed_groups = machine.succeed("id -Gn manageduser").strip()
      print(f"Managed user groups: {managed_groups}")
      assert "wheel" in managed_groups, "manageduser should be in wheel group"

      # Verify existing user is STILL in wheel group after activation
      wheel_members_after = machine.succeed("getent group wheel").strip()
      print(f"Wheel group after activation: {wheel_members_after}")

      existing_groups = machine.succeed("id -Gn zimbatm").strip()
      print(f"Existing user groups after activation: {existing_groups}")

      assert "wheel" in existing_groups, f"zimbatm should STILL be in wheel group after activation, but got: {existing_groups}"
      assert "zimbatm" in wheel_members_after, f"zimbatm should STILL be in wheel group after activation, but wheel group is: {wheel_members_after}"

      print("SUCCESS: Existing group members preserved after userborn activation!")

      # Now test deactivation - only configured members should be removed
      machine.succeed("${toplevel}/bin/deactivate 2>&1 | tee /tmp/output.log")
      machine.succeed("! grep -F 'ERROR' /tmp/output.log")

      print(machine.succeed("journalctl -u userborn.service --no-pager"))

      # Verify wheel group after deactivation
      wheel_members_deactivated = machine.succeed("getent group wheel").strip()
      print(f"Wheel group after deactivation: {wheel_members_deactivated}")

      # zimbatm should STILL be in wheel (pre-existing member preserved)
      assert "zimbatm" in wheel_members_deactivated, f"zimbatm should STILL be in wheel after deactivation, but wheel group is: {wheel_members_deactivated}"

      # manageduser should be REMOVED from wheel (configured member removed)
      assert "manageduser" not in wheel_members_deactivated, f"manageduser should be REMOVED from wheel after deactivation, but wheel group is: {wheel_members_deactivated}"

      existing_groups_after_deactivate = machine.succeed("id -Gn zimbatm").strip()
      print(f"Existing user groups after deactivation: {existing_groups_after_deactivate}")
      assert "wheel" in existing_groups_after_deactivate, f"zimbatm should STILL be in wheel after deactivation, but got: {existing_groups_after_deactivate}"

      print("SUCCESS: Only configured members removed after deactivation, pre-existing members preserved!")
    '';
}

{
  forEachUbuntuImage,
  testModule,
  newConfig,
  system-manager,
  ...
}:

forEachUbuntuImage "prepopulate" {
  modules = [
    (testModule "old")
    ../../examples/example.nix
  ];
  extraPathsToRegister = [ newConfig ];
  testScriptFunction =
    { toplevel, ... }:
    ''
      # Start all machines in parallel
      start_all()

      vm.wait_for_unit("default.target")

      ${system-manager.lib.prepopulateProfileSnippet {
        node = "vm";
        profile = toplevel;
      }}
      vm.systemctl("daemon-reload")

      # Simulate a reboot, to check that the services defined with
      # system-manager start correctly after a reboot.
      # TODO: can we find an easy way to really reboot the VM and not
      # loose the root FS state?
      vm.systemctl("isolate rescue.target")
      # We need to send a return character to dismiss the rescue-mode prompt
      vm.send_key("ret")
      vm.systemctl("isolate default.target")
      vm.wait_for_unit("system-manager.target")

      vm.succeed("systemctl status service-9.service")
      vm.succeed("test -f /etc/baz/bar/foo2")
      vm.succeed("test -f /etc/a/nested/example/foo3")
      vm.succeed("test -f /etc/foo.conf")
      vm.succeed("grep -F 'launch_the_rockets = true' /etc/foo.conf")
      vm.fail("grep -F 'launch_the_rockets = false' /etc/foo.conf")

      ${system-manager.lib.activateProfileSnippet {
        node = "vm";
        profile = newConfig;
      }}
      vm.succeed("systemctl status new-service.service")
      vm.fail("systemctl status service-9.service")
      vm.fail("test -f /etc/a/nested/example/foo3")
      vm.fail("test -f /etc/baz/bar/foo2")
      vm.succeed("test -f /etc/foo_new")

      ${system-manager.lib.deactivateProfileSnippet {
        node = "vm";
        profile = newConfig;
      }}
      vm.fail("systemctl status new-service.service")
      vm.fail("test -f /etc/foo_new")
    '';
}

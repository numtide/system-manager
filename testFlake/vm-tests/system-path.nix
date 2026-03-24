{
  forEachUbuntuImage,
  testModule,
  newConfig,
  system-manager,
  ...
}:

forEachUbuntuImage "system-path" {
  modules = [
    (testModule "old")
    ../../examples/example.nix
  ];
  extraPathsToRegister = [ newConfig ];
  testScriptFunction =
    { toplevel, hostPkgs, ... }:
    ''
      # Start all machines in parallel
      start_all()
      vm.wait_for_unit("default.target")

      vm.fail("bash --login -c '$(which rg)'")
      vm.fail("bash --login -c '$(which fd)'")

      ${system-manager.lib.activateProfileSnippet {
        node = "vm";
        profile = toplevel;
      }}

      vm.wait_for_unit("system-manager.target")
      vm.wait_for_unit("system-manager-path.service")

      #vm.fail("bash --login -c '$(which fish)'")
      vm.succeed("bash --login -c 'realpath $(which rg) | grep -F ${hostPkgs.ripgrep}/bin/rg'")
      vm.succeed("bash --login -c 'realpath $(which fd) | grep -F ${hostPkgs.fd}/bin/fd'")

      ${system-manager.lib.activateProfileSnippet {
        node = "vm";
        profile = newConfig;
      }}

      vm.fail("bash --login -c '$(which rg)'")
      vm.fail("bash --login -c '$(which fd)'")
      vm.succeed("bash --login -c 'realpath $(which fish) | grep -F ${hostPkgs.fish}/bin/fish'")
    '';
}

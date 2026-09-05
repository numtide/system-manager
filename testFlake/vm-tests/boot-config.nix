{
  forEachImage,
  system-manager,
  system,
  ...
}:

let
  emptyConfig = system-manager.lib.makeSystemConfig {
    modules = [
      {
        nixpkgs.hostPlatform = system;
      }
    ];
  };
in
forEachImage "boot-config" {
  modules = [
    (
      { ... }:
      {
        boot.kernel.sysctl = {
          "net.ipv4.ip_forward" = 1;
          "vm.swappiness" = 10;
        };
        boot.kernelModules = [ "veth" ];
      }
    )
  ];
  extraPathsToRegister = [ emptyConfig ];
  testScriptFunction =
    { toplevel, hostPkgs, ... }:
    ''
      start_all()
      vm.wait_for_unit("default.target")

      # Activate empty config: modules-load.d config should not be created
      ${system-manager.lib.activateProfileSnippet {
        node = "vm";
        profile = emptyConfig;
      }}
      vm.wait_for_unit("system-manager.target")

      vm.fail("test -f /etc/modules-load.d/system-manager.conf")
      vm.fail("test -d /etc/systemd/system/systemd-modules-load.service.d")
      # sysctl drop-in exists even without explicit config (upstream defaults)
      vm.succeed("test -e /etc/systemd/system/systemd-sysctl.service.d/overrides.conf")

      # Activate with kernel modules: config should exist
      ${system-manager.lib.activateProfileSnippet {
        node = "vm";
        profile = toplevel;
      }}
      vm.wait_for_unit("system-manager.target")

      vm.succeed("test -f /etc/modules-load.d/system-manager.conf")
      vm.succeed("grep -q veth /etc/modules-load.d/system-manager.conf")
      vm.succeed("test -e /etc/systemd/system/systemd-modules-load.service.d/overrides.conf")

      # Verify sysctl config file
      vm.succeed("test -f /etc/sysctl.d/60-nixos.conf")
      vm.succeed("grep -q net.ipv4.ip_forward /etc/sysctl.d/60-nixos.conf")
      vm.succeed("grep -q vm.swappiness /etc/sysctl.d/60-nixos.conf")
      vm.succeed("test -e /etc/systemd/system/systemd-sysctl.service.d/overrides.conf")

      ip_forward = vm.succeed("sysctl -n net.ipv4.ip_forward").strip()
      assert ip_forward == "1", f"Expected net.ipv4.ip_forward=1, got {ip_forward}"

      swappiness = vm.succeed("sysctl -n vm.swappiness").strip()
      assert swappiness == "10", f"Expected vm.swappiness=10, got {swappiness}"

      vm.succeed("lsmod | grep -q veth")
    '';
}

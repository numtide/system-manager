{ lib, ... }:
{
  options = {
    # Stubs for options referenced by programs/ssh.nix but not available in
    # system-manager.
    services.xserver.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      internal = true;
    };

    services.openssh.settings = lib.mkOption {
      type = lib.types.attrsOf lib.types.raw;
      default = { };
      internal = true;
    };

    environment.variables = lib.mkOption {
      type = lib.types.attrsOf lib.types.str;
      default = { };
      internal = true;
    };

    systemd.user.services = lib.mkOption {
      type = lib.types.attrs;
      default = { };
      internal = true;
    };
  };

  config = {
    services.openssh.settings.X11Forwarding = lib.mkDefault false;

    programs.ssh.enableAskPassword = lib.mkDefault false;
    programs.ssh.setXAuthLocation = lib.mkDefault false;
    programs.ssh.startAgent = lib.mkDefault false;
    programs.ssh.systemd-ssh-proxy.enable = lib.mkDefault false;

    environment.etc."ssh/ssh_config".replaceExisting = lib.mkDefault true;
    environment.etc."ssh/ssh_known_hosts".replaceExisting = lib.mkDefault true;
  };
}

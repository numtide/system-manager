# Managing Systemd Service Modules

Custom or existing systemd services can be created and configured with `system-manager`.
This topic is relative to `system-manager`, and is more directly correlated with `systemd`
and `nix`. Since it requires knowledge of both we cover it minimally here, however, the
following resources are a better reference for creating services with nix which can be
used by the `system-manager` module.

- [Official `systemd.service` docs](https://www.freedesktop.org/software/systemd/man/latest/systemd.service.html)
- [Systemd NixOS Wiki](https://wiki.nixos.org/wiki/Systemd)
- [Search Engine for NixOS configuration options](https://mynixos.com/search?q=systemd) -- helpful for quickly finding attribute bindings for systemd

## Example systemd module

The following is a simple systemd timer which continues to capture timestamps into a log file
once every minute, after the timer unit itself is activated.

Further configurations settings for creating a `systemd.timer` can be found in the [official `systemd.timer` docs](https://www.freedesktop.org/software/systemd/man/latest/systemd.timer.html).

```nix
{ pkgs, ... }:
{
  systemd = {
    timers.simple-timer = {
      wantedBy = [ "timers.target" ];
      timerConfig = {
        OnActiveSec = "10"; # Defines a timer relative to the moment the timer unit itself is activated.
        OnCalendar = "minutely";
        Unit = "simple-timer.service";
        Persistent = true; # Stores the last activated time to disk.
      };
    };
    services.simple-timer = {
      serviceConfig.Type = "oneshot";
      script = ''
        echo "Time: $(date)." >> /tmp/simple-timer.log
      '';
    };
  };
}
```

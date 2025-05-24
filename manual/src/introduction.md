# Introduction

`system-manager` is a command line tool that uses system configurations written in the nix language
to manage installation of system services on Linux distributions, such as Ubuntu, in a similar manner to
`nixos-rebuild` on NixOS systems.

## Hello, System Manager

The following snippet of nix code...

```nix
{ lib, pkgs, ... }:
{
  config.systemd.services.say-hello = {
    description = "say-hello";
    enable = true;
    wantedBy = [ "system-manager.target" ];
    serviceConfig = {
      Type = "oneshot";
      RemainAfterExit = true;
    };
    script = ''
      ${lib.getBin pkgs.hello}/bin/hello
    '';
  };
}
```

produces a systemd service file, `/etc/systemd/system/say-hello.service`:

```service
[Unit]
Description=say-hello

[Service]
Environment="PATH=..."
ExecStart=/nix/store/...-unit-script-say-hello-start/bin/say-hello-start
RemainAfterExit=true
Type=oneshot

[Install]
WantedBy=system-manager.target
```

When the service is enabled it results in `"Hello, World!"` in the journal of the service `say-hello.service`:

```sh
systemctl status say-hello.service
● say-hello.service - say-hello
     Loaded: loaded (/etc/systemd/system/say-hello.service; enabled; vendor preset: enabled)
     Active: active (exited) since Wed 2025-05-21 09:39:24 PDT; 11min ago
   Main PID: 41644 (code=exited, status=0/SUCCESS)
        CPU: 3ms

May 21 09:39:24 ubuntu systemd[1]: Starting say-hello...
May 21 09:39:24 ubuntu say-hello-start[41646]: Hello, world!
May 21 09:39:24 ubuntu systemd[1]: Finished say-hello.
```

> Context omitted for clarity, see [Example Configuration](./usage/example-configuration.md)
> for a fully functioning use case.

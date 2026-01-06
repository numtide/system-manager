# Timer

This example demonstrates how to install a systemd timer that runs every minute.

## Configuration

### flake.nix

```nix
{
  description = "Standalone System Manager configuration";

  inputs = {
    # Specify the source of System Manager and Nixpkgs.
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    system-manager = {
      url = "github:numtide/system-manager";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      system-manager,
      ...
    }:
    let
      system = "x86_64-linux";
    in
    {
      systemConfigs.default = system-manager.lib.makeSystemConfig {
        # Specify your system configuration modules here, for example,
        # the path to your system.nix.
        modules = [ ./system.nix ];

        # Optionally specify extraSpecialArgs and overlays
      };
    };
}
```

### system.nix

```nix
{ pkgs, ... }:
{
  nixpkgs.hostPlatform = "x86_64-linux";

  # Define the timer that fires every minute
  systemd.timers.simple-timer = {
    enable = true;
    description = "Simple timer that runs every minute";
    wantedBy = [ "timers.target" ];
    timerConfig = {
      OnCalendar = "minutely";
      Persistent = true;
    };
  };

  # Define the service that the timer triggers
  systemd.services.simple-timer = {
    enable = true;
    description = "Simple timer service";
    serviceConfig = {
      Type = "oneshot";
      ExecStart = "${pkgs.bash}/bin/bash -c 'echo \"Timer fired at $(date)\" >> /tmp/simple-timer.log'";
    };
  };
}
```

## Usage

```bash
# Activate the configuration
nix run 'github:numtide/system-manager' -- switch --flake /path/to/this/example --sudo
```

Then restart the system; the timer will start automatically.

```bash
# View the file created every one minute
cat /tmp/simple-timer.log
```

## Notes

- The timer will not start automatically until you reboot the system. If you wish to start it manually, you can do so by typing:

```bash
sudo systemctl start simple-timer.timer
```

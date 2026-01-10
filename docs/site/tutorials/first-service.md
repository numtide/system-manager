# Your First Systemd Service

This tutorial walks you through creating and managing a systemd service with System Manager.

## Prerequisites

- System Manager initialized (see [Getting Started](getting-started.md))
- Basic familiarity with Nix syntax

## What You'll Build

A simple "hello world" systemd service that runs when System Manager activates your configuration.

## Step 1: Create a Service Module

In your `~/.config/system-manager` folder, create a file called `hello-service.nix`:

```nix
{ lib, pkgs, ... }:
{
  config = {
    nixpkgs.hostPlatform = "x86_64-linux";

    systemd.services.say-hello = {
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
  };
}
```

Let's break down what each part does:

- `systemd.services.say-hello` - Creates a service named "say-hello"
- `enable = true` - Activates the service
- `wantedBy = [ "system-manager.target" ]` - Starts the service when System Manager activates
- `Type = "oneshot"` - Runs once and exits
- `RemainAfterExit = true` - Keeps the service marked as "active" after the script finishes
- `script` - The actual command to run

## Step 2: Add the Module to Your Flake

Edit your `flake.nix` and add the new module:

```nix
modules = [
    ./system.nix
    ./hello-service.nix
];
```

## Step 3: Apply the Configuration

Run System Manager to activate your new service:

```sh
nix run 'github:numtide/system-manager' -- switch --flake . --sudo
```

## Step 4: Verify It Worked

Check the journal to see your service ran:

```sh
journalctl -n 20
```

You should see output like:

```log
Nov 18 12:12:51 my-ubuntu systemd[1]: Starting say-hello.service - say-hello...
Nov 18 12:12:51 my-ubuntu say-hello-start[3488278]: Hello, world!
Nov 18 12:12:51 my-ubuntu systemd[1]: Finished say-hello.service - say-hello.
```

## Step 5: Check the Generated Service File

System Manager created a systemd unit file at `/etc/systemd/system/say-hello.service`. You can view it with:

```sh
cat /etc/systemd/system/say-hello.service
```

## Next Steps

- Learn about [service configuration options](../reference/modules.md#systemdservices) in the reference
- See the [timer example](../examples/timer.md) for scheduled services
- Explore the [nginx example](../examples/nginx.md) for a real-world service

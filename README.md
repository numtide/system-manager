<div align="center">

# system-manager

<img src="system-manager.svg" height="150"/>

**Manage system config using nix on any distro**

*A <a href="https://numtide.com/">numtide</a> project.*

<p>
<a href="https://github.com/numtide/system-manager/actions/workflows/update-flake-lock.yml"><img src="https://github.com/numtide/system-manager/actions/workflows/update-flake-lock.yml/badge.svg"/></a>
<a href="https://app.element.io/#/room/#home:numtide.com"><img src="https://img.shields.io/badge/Support-%23numtide-blue"/></a>
</p>

</div>

This project provides a basic method to manage system configuration using [Nix][nixos]
on any Linux distribution.
It builds on the many modules that already exist in [NixOS][nixos].

*Warning*: System Manager is a work in progress, you can expect things not to work or to break.

[nixos]: https://nixos.org

## Usage

### Install Nix

In order to use System Manager, you will first need to install Nix.
You can either use your distro's package manager, or use one of the available options
to install Nix.

- [Official Nix Installer][official-installer] - The canonical source for installing nix.
- [Determinate Nix Installer][detsys-installer] - A wrapper around the official installer that has SELinux support, and enables flake features by default.

> **Tip:** Some Nix users have had difficulty installing Nix on Windows Subsystem for Linux (WSL) using the official installer. If you're using WSL, we recommend using Determinate Nix installer.

> **Note:** Determinate Systems installer has the option for the official Nix as well as Determinate's own variant of Nix (Determinate Nix). It will prompt you for which one you want to install. Although we've used Determinate Nix several times with System Manager and it has worked fine, we have not done any official testing on it. We therefore recommend you use the official Nix if installing via the Determinate Nix Installer.

[official-installer]: https://nixos.org/download.html
[detsys-installer]: https://github.com/DeterminateSystems/nix-installer

## Usage with flakes NixOS Style

Nix has the philosophy:

> "If it's not in the config, it's not real."

Normally on Ubuntu and other distros that use systemd, you enable, start, stop, or disable a system service by typing:

```
sudo systemctl enable foo.service   # to enable the service to start on boot
sudo systemctl start foo.service    # to start the service immediately
sudo systemctl stop foo.service     # to stop the service
sudo systemctl disable foo.service  # to prevent it from starting on boot
```

This approach is called "imperative" — meaning you're telling the system what to do right now, and the system will remember your choice by mutating internal state (like creating symlinks in /etc/systemd/system/).

But this method has some downsides:

* There's no central place where your desired system state is defined.

* If someone else logs in and changes something, there's no record.

* You can’t easily back up, reproduce, or version-control your system configuration.


Now let's consider how this is done on NixOS (even though you're using Ubuntu or similar). With NixOS, you manage services declaratively using configuration files tracked in Git. In a flake-based setup on NixOS, you define services like this:

```
{
  systemd.services.foo.enable = true;
}
```

Then you apply the config with:
```
sudo nixos-rebuild switch --flake .

```

Nix builds the desired system state from your flake and makes it real — enabling, disabling, and managing services automatically. No imperative commands. No hidden state. Just one source of truth: your config.

Now of course, this assumes you have a service file that's already been created. On a traditional system such as Ubuntu, you're often stuck having to create one yourself (or downloading one), and saving it in a path like `/etc/systemd/system/foo.service`. Then you use the systemd commands we talked about earlier.

NixOS on the other hand, handles this declaratively in which you define the entire system configuration in your flake file, like so (this is just part of the code):

```nix
systemd.services.foo = {
  enable = true;
  description = "My custom service";
  serviceConfig = {
    ExecStart = "/path/to/executable";
    Restart = "always";
  };
  wantedBy = [ "multi-user.target" ];
};
```

Then on NixOS, to rebuild your system with this flake you type:

```
sudo nixos-rebuild switch --flake .
```

Nix takes care of everything — writing the unit file, reloading systemd, enabling it on boot, and starting it if needed.

Also, notice the format of the command: we run `nixos-rebuild`, followed by the `switch` subcommand, and then pass `--flake .`, where the dot (.) refers to the current directory containing your flake.

Putting your service declarations inside a flake file offers huge advantages over manually creating and managing system service files. Your entire setup — from services to packages to user settings — can live in a Git repo, making it easy to track changes, collaborate, and "version-control" your system like code. Need to set up a second machine just like the first? Just clone the repo and run a single command and you're good to go. And thanks to Nix’s built-in rollback capabilities, you can safely experiment, knowing you can always revert to a known-good configuration.

And... now with system manager you can do the same with Ubuntu and other systemd-based repos.

## Meet System Manager: Manage systemd the NixOS way 

With System Manager, you can configure system services almost exactly the way you do in NixOS. You create flakes that declare your intention, and you run a command that's effectively the same as the NixOS way.

Let's start with a basic service. We'll install TightVNC, which is a VNC server allowing you to log in with a GUI and desktop manager.

**[Next: I have two examples ready that I'm going to describe -- a TightVNC service, and a custom node.js service]** Following is the original readme









### Defining the configuration

A basic Nix flake using System Manager would look something like this:

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    system-manager = {
      url = "github:numtide/system-manager";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, flake-utils, nixpkgs, system-manager }: {
    systemConfigs.default = system-manager.lib.makeSystemConfig {
      modules = [
        ./modules
      ];
    };
  };
}
```

And you would then put your System Manager modules in the `modules` directory,
which should contain a `default.nix` file which functions as the entrance point.

A simple System Manager module could look something like this:

```nix
{ config, lib, pkgs, ... }:

{
  config = {
    nixpkgs.hostPlatform = "x86_64-linux";

    environment = {
      etc = {
        "foo.conf".text = ''
          launch_the_rockets = true
        '';
      };
      systemPackages = [
        pkgs.ripgrep
        pkgs.fd
        pkgs.hello
      ];
    };

    systemd.services = {
      foo = {
        enable = true;
        serviceConfig = {
          Type = "oneshot";
          RemainAfterExit = true;
        };
        wantedBy = [ "system-manager.target" ];
        script = ''
          ${lib.getBin pkgs.hello}/bin/hello
          echo "We launched the rockets!"
        '';
      };
    };
  };
}
```

### Activating the configuration

Once the configuration is defined, you can activate it using the `system-manager` CLI:
```sh
nix run 'github:numtide/system-manager' -- switch --flake '.'
```

### Reproducibility

By design flakes run in [pure evaluation mode](https://wiki.nixos.org/wiki/Flakes#Making_your_evaluations_pure).
In some cases you may not want this. To run an impure evaluation of the flake, add the following option to your command:
```sh
--nix-option pure-eval false
```


## Currently supported features

Currently it is possible to configure files under `/etc/` and systemd services.
More features may follow later.

### Supported Systems

System Manager is currently only supported on NixOS and Ubuntu. However, it can be used on other distributions by enabling the following:

```nix
{
  config = {
    system-manager.allowAnyDistro = true;
  }
}
```

> [!WARNING]
> This is unsupported and untested. Use at your own risk.

## Commercial support

Looking for help or customization?

Get in touch with Numtide to get a quote. We make it easy for companies to
work with Open Source projects: <https://numtide.com/contact>

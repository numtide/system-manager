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

System Manager brings the power of NixOS-style declarative configuration to any Linux system. Describe what your system should look like, by specifying packages, services, and settings—in Nix, then apply it safely and atomically with a single command. Each change is reproducible and rollback-ready, just like NixOS generations.

If you're familiar with Home Manager, this of it as similar to Home Manager but for your entire machine. Whereas Home Manager manages user environments, System Manager manages the full system, starting at root-level configurations, packages, and services, using the same reliable, Nix-based model.

System Manager builds on the many modules that already exist in [NixOS][nixos].

[nixos]: https://nixos.org

## Quick Example

Assume you're using a non-NixOS distrubution (such as Ubuntu) and you have Nix already installed. In a folder create a file called `flake.nix` with the following:

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    system-manager = {
      url = "github:numtide/system-manager";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, system-manager }: {
    systemConfigs.default = system-manager.lib.makeSystemConfig {
      modules = [
        ./modules
      ];
    };
  };
}
```

In that folder create a subfolder called `modules`; in `modules` create a file called `default.nix` with the following:

```nix
{ pkgs, ... }:
{
  nixpkgs.hostPlatform = "x86_64-linux";
  
  environment.systemPackages = with pkgs; [
    btop          # Beautiful system monitor
    bat           # Modern 'cat' with syntax highlighting
  ];
}

```
This will install a couple of tools on your system, btop and bat. To do so, execute system manager with sudo using the nix command (assuming you have experimental features nix-command and flakes turned on):

```
sudo env PATH="$PATH" nix run 'github:numtide/system-manager' -- switch --flake .
```

Notice we're passing the current PATH environment into sudo so that the elevated shell can locate the nix command.

Also, note that you might need to enable nix-commands and flakes:

```
sudo env PATH="$PATH" nix --extra-experimental-features 'nix-command flakes'  run 'github:numtide/system-manager' -- switch --flake .
```

## Full Installation and setup

### Install Nix

System manager itself does not need to be installed; but, you do need to install Nix. (However, you can optionally install system-manager locally if you prefer.)

To install Nix, you can either use your distro's package manager, or use one of the following available options to install Nix.

- [Official Nix Installer][official-installer] - The canonical source for installing nix.
- [Determinate Nix Installer][detsys-installer] - A wrapper around the official installer that has SELinux support, and enables flake features by default.

> **Tip:** Some Nix users have had difficulty installing Nix on Windows Subsystem for Linux (WSL) using the official installer. If you're using WSL, we recommend using Determinate Nix installer.

[official-installer]: https://nixos.org/download.html
[detsys-installer]: https://github.com/DeterminateSystems/nix-installer

## Usage with flakes NixOS Style

[THE FOLLOWING IS AN EARLIER DRAFT UNDERGOING A COMPLETE REWRITE.]

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

As mentioned above, a basic Nix flake using System Manager would look like this:

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

This allows you to place your configuration in the modules directory. The configuration is very similar to what you would use in NixOS.


Here's an example configuration that creates file in etc, and installs two packages, btop and bat; it also instsalls a one-shot system service called hello.

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
        pkgs.btop
        pkgs.bat
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

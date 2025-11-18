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

System Manager brings the power of NixOS-style declarative configuration to other Linux distributions. Describe what your system should look like, by specifying packages, services, and settings all in Nix, then apply it safely and atomically with a single command. Each change is reproducible and soon will be rollback-ready, just like NixOS generations.

If you're familiar with Home Manager, think of it as being similar, but for your entire machine. Whereas Home Manager manages user environments, System Manager manages the entire system, starting at root-level configurations, packages, and services, using the same reliable, Nix-based model.

System Manager builds on the many modules that already exist in [NixOS].

## Quick Example to Get Started

We will assume you're using a non-NixOS distrubution (such as Ubuntu) and you have Nix already installed.

System Manager has an "init" subcommand that can build a set of starting files for you. By default, it places the files in `~/.config/system-manager`. You can run this init subcommand by typing:

```
nix run 'github:numtide/system-manager' -- init
```

If you see an error regarding experimental nix features, instead type the following:

```
nix run 'github:numtide/system-manager' --extra-experimental-features 'nix-command flakes' -- init
```

> [!Tip]
> If you still have problems running this step, check out our full Getting Started guide, which includes how to handle issues of running as root, and whether you've installed Nix to be used by a single user.

> [!Note]
> You can optionally run the above under `sudo`, which will place the files under `/root/.config/system-manager`. You might need to pass the path, depending on how you installed Nix:
> `sudo env PATH="$PATH" nix run 'github:numtide/system-manager' -- init`

You will now have two files under `~/.config/system-manager` (or `/root/.config/system-manager` if you ran the above under sudo):

- flake.nix
- system.nix

Go ahead and switch to the folder:

```
cd ~/.config/system-manager
```

(Or to the root equivalent.)

Here is what flake.nix looks like:

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

For these examples, we won't use the `system.nix` file; we'll create separate configuration files for each example.

Find the following line in `flake.nix`:

```
        modules = [ ./system.nix ];
```

and change it to this:

```
        modules = [ ./cli_tools.nix ];
```

Create a file in the same folder called `cli_tools.nix` and add the following into it:

```nix
{ pkgs, ... }:
{
  config = {
    nixpkgs.hostPlatform = "x86_64-linux";
    
    environment.systemPackages = with pkgs; [
      btop          # Beautiful system monitor
      bat           # Modern 'cat' with syntax highlighting
    ];
  };
}

```

This specifies a configuration that includes `btop` and `bat` to be installed on the system. To do so, execute System Manager with `sudo` using the nix command (assuming you have experimental features nix-command and flakes turned on):

```
sudo env PATH="$PATH" nix run 'github:numtide/system-manager' -- switch --flake .
```

Notice we're passing the current `PATH` environment into `sudo` so that the elevated shell can locate the `nix` command.

Also, note that you might need to enable `nix-commands` and `flakes` if you don't already have them set:

```
sudo env PATH="$PATH" nix --extra-experimental-features 'nix-command flakes' run 'github:numtide/system-manager' -- switch --flake .
```

> [!Note]
> The first time you run System Manager, it will update your path by adding an entry in the /etc/profile.d folder. For this change to take effect, you need to log out and then log back in. However, if you don't want to log out, you can source the file:
> `source /etc/profile.d/system-manager-path.sh`

And now the two commands `btop` and `bat` should be available on your system:

```
$ btop --version
btop version: 1.4.5
Compiled with: g++ (14.3.0)
Configured with: cmake -DBTOP_STATIC=OFF -DBTOP_GPU=ON
$ bat --version
bat 0.26.0
```

Want to remove a package? Simply remove it or comment it out in the `cli_tools.nix` file, and run it again. For example, if you want to remove `bat`, simply update the `default.nix` to the following:

```nix
{ pkgs, ... }:
{
  nixpkgs.hostPlatform = "x86_64-linux";
  
  environment.systemPackages = with pkgs; [
    btop          # Beautiful system monitor
    # bat         # Comment out or remove
  ];
}
```

## Regarding the Error

You might notice an error that looks like this:

`[2025-11-17T15:06:46Z ERROR system_manager::activate::etc_files] Error while trying to link directory /etc/.system-manager-static/nix: Unmanaged path already exists in filesystem, please remove it and run system-manager again: /etc/nix/nix.conf`

You can safely ignore it; or, you can allow System Manager to take control of `nix.conf`. If you choose to have System Manager take control of `nix.conf`, rename `nix.conf` to a backup name, such as `nix_conf_backup`, and run System Manager again. Note, however, that if you had settings in your `nix.conf` file, they might not appear in the new file System Manager generates. For that read the following section, [Adding in Experimental Features](#adding-in-experimental-features).

## Adding in Experimental Features

It's possible that you had a `nix.conf` file in `/etc/nix` that had experimental features set. And if you allowed System Manager to take control of that file, your setting will likely be gone. But that's okay; you now have control of your system right from the `flake.nix` file. You can add experimental features inside the modules list like so:

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
	
        modules = [
            # Place additional settings here:
            {
                nix.settings.experimental-features = "nix-command flakes";
            }
            ./cli_tools.nix 
        ];

        # Optionally specify extraSpecialArgs and overlays
      };
    };
}
```

Then re-run System Manager and your changes will take effect; now you should have the two experimental features set, `nix-command` and `flakes`.

# Full Installation and setup

## Install Nix

System manager itself does not need to be installed; but, you do need to install Nix. (However, you can optionally install system-manager locally if you prefer.)

To install Nix, you can either use your distro's package manager, or use one of the following available options to install Nix.

- [Official Nix Installer][official-installer] - The canonical source for installing nix.
- [Determinate Nix Installer][detsys-installer] - A wrapper around the official installer that has
  SELinux support, and enables flake features by default.

> [!Tip]
> Some Nix users have had difficulty installing Nix on Windows Subsystem for Linux (WSL) using the official installer. If you're using WSL, we recommend using Determinate Nix installer.

## Example: Configuring System Services

The following example demonstrates how to specify a system service and activate it.

Update your flake.nix file to include a new file in the modules list, which we'll call `say_hello.nix`:

```
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
	
        modules = [
            {
                nix.settings.experimental-features = "nix-command flakes";
            }
            ./cli_tools.nix 
            ./say_hello.nix
        ];

        # Optionally specify extraSpecialArgs and overlays
      };
    };
}

```

Then create the file called `say_hello.nix` and add the following to it:

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

Activate it using the same nix command as earlier:

```
sudo env PATH="$PATH" nix run 'github:numtide/system-manager' -- switch --flake .
```

This will create a system service called `say-hello` (which comes from the line `config.systemd.services.say-hello`) in a unit file at `/etc/systemd/system/say-hello.service` with the following inside it:

```
[Unit]
Description=say-hello

[Service]
Environment="PATH=/nix/store/xs8scz9w9jp4hpqycx3n3bah5y07ymgj-coreutils-9.8/bin:/nix/store/qqvfnxa9jg71wp4hfg1l63r4m78iwvl9-findutils-4.10.0/bin:/nix/store/22r4s6lqhl43jkazn51f3c18qwk894g4-gnugrep-3.12/bin:
/nix/store/zppkx0lkizglyqa9h26wf495qkllrjgy-gnused-4.9/bin:/nix/store/g48529av5z0vcsyl4d2wbh9kl58c7p73-systemd-minimal-258/bin:/nix/store/xs8scz9w9jp4hpqycx3n3bah5y07ymgj-coreutils-9.8/sbin:/nix/store/qqvfn
xa9jg71wp4hfg1l63r4m78iwvl9-findutils-4.10.0/sbin:/nix/store/22r4s6lqhl43jkazn51f3c18qwk894g4-gnugrep-3.12/sbin:/nix/store/zppkx0lkizglyqa9h26wf495qkllrjgy-gnused-4.9/sbin:/nix/store/g48529av5z0vcsyl4d2wbh9
kl58c7p73-systemd-minimal-258/sbin"
ExecStart=/nix/store/d8rjglbhinylg8v6s780byaa60k6jpz1-unit-script-say-hello-start/bin/say-hello-start 
RemainAfterExit=true
Type=oneshot

[Install]
WantedBy=system-manager.target
```

> [!Tip]
> Compare the lines in the say-hello.service file with the default.nix file to see where each comes from.

You can verify that it ran by running journalctl:

```
journalctl -n 20
```

and you can find the following output in it:

```
Nov 18 12:12:51 my-ubuntu systemd[1]: Starting say-hello.service - say-hello...
Nov 18 12:12:51 my-ubuntu say-hello-start[3488278]: Hello, world!
Nov 18 12:12:51 my-ubuntu systemd[1]: Finished say-hello.service - say-hello.
```

> [!Note]
> If you remove the `./cli_tools.nix` line from the flake.nix, System Manager will see that the configuration changed and that `btop` and `bat` are no longer in the configuration. As such, it will uninstall them. This is normal and expected behavior.

## Example: Creating files in the /etc folder

Oftentimes, when you're creating a system service, you need to create a configuration file in the `/etc` directory that accompanies the service. System manager allows you to do that as well.

Add another line to your `flake.nix` file, this time for `./sample_etc.nix`:

```
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
	
        modules = [
            {
                nix.settings.experimental-features = "nix-command flakes";
            }
            ./cli_tools.nix 
	    ./say_hello.nix
            ./sample_etc.nix
        ];

        # Optionally specify extraSpecialArgs and overlays
      };
    };
}
```

Then, create the `sample_etc.nix` file with the following into it:

```
{ lib, pkgs, ... }:
{
  config = {
    nixpkgs.hostPlatform = "x86_64-linux";
    
    environment.etc = {
      sample_configuration = {
        text = ''
          This is some sample configuration text
        '';
      };
    };
  };
}
```

Run it as usual, and you should see the file now exists:

```
sudo env PATH="$PATH" nix run 'github:numtide/system-manager' -- switch --flake .

ls /etc -ltr
```

which displays the following:

```
lrwxrwxrwx  1 root  root  45 Nov 13 15:19 sample_configuration -> ./.system-manager-static/sample_configuration
```

And you can view the file:

```
cat /etc/sample_configuration
```

which prints out:

```
This is some sample configuration text
```

### Supported Systems

System Manager is currently only supported on NixOS and Ubuntu. However, it can be used on other distributions by enabling the following:

```nix
{
  config = {
    system-manager.allowAnyDistro = true;
  }
}
```

## Commercial support

Looking for help or customization?

Get in touch with Numtide to get a quote. We make it easy for companies to
work with Open Source projects: <https://numtide.com/contact>

[detsys-installer]: https://github.com/DeterminateSystems/nix-installer
[nixos]: https://nixos.org
[official-installer]: https://nixos.org/download.html

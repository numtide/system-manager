# <img alt="System Manager" src="https://banner.numtide.com/banner/numtide/system-manager.svg">

<p>
<a href="https://github.com/numtide/system-manager/actions/workflows/update-flake-lock.yml"><img src="https://github.com/numtide/system-manager/actions/workflows/update-flake-lock.yml/badge.svg"/></a>
<a href="https://app.element.io/#/room/#home:numtide.com"><img src="https://img.shields.io/badge/Support-%23numtide-blue"/></a>
</p>

System manager is a tool to configure Linux machines. Unlike Chef, Puppet and Ansible, it only controls a small subset, and most of its changes are done in an immutable layer, thanks to the power of Nix.

Using NixOS-style declarative configurations, you describe what your system should look like, by specifying packages, services, and settings all in Nix, then apply it safely and atomically with a single command. Each change is reproducible, just like NixOS generations.

You don't need to be an expert in Nix to use it, as its syntax is straightforward. But if you're familiar with Nix and Home Manager, think of System Manager as being similar, but for your entire machine. Whereas Home Manager manages user environments, System Manager manages the entire system, starting at root-level configurations, packages, and services, using the same reliable, Nix-based model.

System Manager builds on the many modules that already exist in [NixOS].

# Full Documentation

You can find the [full documentation here](https://system-manager.net/main/).

## Quick Example to Get Started

We will assume you're using a non-NixOS distribution (such as Ubuntu) and you have Nix already installed, with flakes enabled.

System Manager has an "init" subcommand that can build a set of starting files for you. By default, it places the files in `~/.config/system-manager`. You can run this init subcommand by typing:

```
nix run 'github:numtide/system-manager' -- init
```

If you see an error regarding experimental nix features, instead type the following:

```
nix run 'github:numtide/system-manager' --extra-experimental-features 'nix-command flakes' -- init
```

> [!Tip]
> If you still have problems running this step, check out our full [Getting Started Guide](https://system-manager.net/main/tutorials/getting-started/), which includes how to handle issues of running as root, and whether you've installed Nix to be used by a single user.

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

Here is what `flake.nix` looks like. (Note: If you enabled experimental features from the command line rather than through `/etc/nix/nix.conf`, this file might not exist; you can create it manually and copy the following into it.)

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

This specifies a configuration that includes `btop` and `bat` to be installed on the system. To do so, execute System Manager using the nix command (assuming you have experimental features nix-command and flakes turned on):

```
nix run 'github:numtide/system-manager' -- switch --flake . --sudo
```

Also, note that you might need to enable `nix-commands` and `flakes` if you don't already have them set in `/etc/nix/nix.conf`:

```
nix --extra-experimental-features 'nix-command flakes' run 'github:numtide/system-manager' -- switch --flake . --sudo
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

## Supported Systems

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

## Supported Nix

Nix should be installed with the [nix-installer](https://github.com/NixOS/nix-installer).
System manager is tested against Nix 2.32 and above installed via nix-installer.

## Commercial support

Looking for help or customization?

Get in touch with Numtide to get a quote. We make it easy for companies to work with Open Source projects: <https://numtide.com/contact>

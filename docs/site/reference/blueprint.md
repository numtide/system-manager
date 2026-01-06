# Blueprint

Blueprint is an opinionated library that maps a standard folder structure to flake outputs, allowing you to divide up your flake into individual files across these folders. This allows you to modularize and isolate these files so that they can be maintained individually and even shared across multiple projects.

Blueprint has built-in support for System Manager, which means:

* You do not need to call `system-manager.lib.makeSystemConfig`; Blueprint calls this for you
* You must follow Blueprint's folder structure by placing your files under the `hosts` folder, and you must name your files `system-configuration.nix`.
* You can have multiple folders under the `hosts` folder (but one level deep), and you can access these using the standard nix specifier, e.g. `.#folder-name`.

In this section we show you how to use Blueprint with System Manager.

Blueprint provides its own initialization that you can start with if you don't already have a `flake.nix` file using Blueprint. The command to type is:

```sh
nix flake init -t github:numtide/blueprint
```

This results in the following flake:

```nix
{
  description = "Simple flake with a devshell";

  # Add all your dependencies here
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs?ref=nixos-unstable";
    blueprint.url = "github:numtide/blueprint";
    blueprint.inputs.nixpkgs.follows = "nixpkgs";
  };

  # Load the blueprint
  outputs = inputs: inputs.blueprint { inherit inputs; };
}
```

Now add System Manager to its inputs section:

```nix
    system-manager = {
        url = "github:numtide/system-manager";
        inputs.nixpkgs.follows = "nixpkgs";
    };
```

Next, create a folder called `hosts`, and under that a folder called `default`:

```sh
mkdir -p hosts/default
cd hosts/default
```

Inside `default` is where you'll put your configuration file.

**This configuration file must be named `system-configuration.nix`.**

For example, here's a configuration file that installs `bat`:

```nix
{ lib, pkgs, ... }:
{
  config = {
    nixpkgs.hostPlatform = "x86_64-linux";
    environment = {
      # Packages that should be installed on a system
      systemPackages = [
        pkgs.bat
      ];
    };
  };
}
```

!!! Note
   Notice that we need to include `nixpkgs.hostPlatform` in this file, as there's no place to include it in the parent `flake.nix` file.

Now return to the folder two levels up (the one containing `flake.nix`) and you can run System Manager:

```sh
nix run 'github:numtide/system-manager' -- switch --flake . --sudo
```

!!! Remember
    As mentioned elsewhere, if this is the first time running System Manager on this computer, you'll need to log out and log back in to pick up the new path.

Then you should find `bat` on your path:

```console
$ which bat
/run/system-manager/sw/bin//bat
```

The default folder is called `default`; you can also refer to folders by name as mentioned earlier.

If, for example, under the `hosts` folder you have a folder called `tree`, and inside `tree` you create a file called `system-configuration.nix` with the following contents:

```nix
{ lib, pkgs, ... }:
{
  config = {
    nixpkgs.hostPlatform = "x86_64-linux";
    environment = {
      # Packages that should be installed on a system
      systemPackages = [
        pkgs.tree
      ];
    };
  };
}
```

Then you can choose to install `tree` by specifying the `tree` folder like so:

```sh
nix run 'github:numtide/system-manager' -- switch --flake '.#tree' --sudo
```

## Using multiple configuration files with Blueprint

If you want to load multiple configuration files at once, you can create a special `system-configuration.nix` file that loads multiple files from a `modules` folder (or any name you choose). To accomplish this, create a folder under `hosts`; for example, you might name it `cli-tools`. Starting in the folder with `flake.nix`:

```sh
mkdir -p hosts/cli-tools/modules
```

Then, inside the `cli-tools` folder, create a `system-configuration.nix` file with the following:

```nix
{ config, lib, pkgs, ... }:
{
  # Import all your modular configs - they auto-merge!
  imports = [
    ./modules/tldr.nix
    ./modules/cowsay.nix
  ];

  # Base configuration that applies to everything
  config = {
    nixpkgs.hostPlatform = "x86_64-linux";
  };
}
```

(Notice this time we can put the `nixpkgs.hostPlatform` in a single place. As such we won't need it in the configuration files.)

Now move into the `modules` folder:

```sh
cd modules
```

And create two files here:

tldr.nix:

```nix
{ lib, pkgs, ... }:
{
  config = {
    environment = {
      # Packages that should be installed on a system
      systemPackages = [
        pkgs.tldr
      ];
    };
  };
}
```

cowsay.nix:
```nix
{ lib, pkgs, ... }:
{
  config = {
    environment = {
      # Packages that should be installed on a system
      systemPackages = [
        pkgs.cowsay
      ];
    };
  };
}
```

Now you can return to the top level where your `flake.nix` file is and run these two configuration files:

```sh
nix run 'github:numtide/system-manager' -- switch --flake '.#cli-tools' --sudo
```

This means if you want to include various recipes, you can easily do so.

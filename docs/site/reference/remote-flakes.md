# Remote Flakes

Instead of saving your System Manager configuration files locally, you can optionally keep them in a remote Git repository, such as on GitHub.

!!! Note
    This is a great option if you plan to use the files on multiple machines.

In order to store them on a remote repo, it's imperative that you keep your `flake.lock` file up to date.

## What's a `flake.lock` file?

A `flake.lock` file is a JSON file that stores the exact versions of all the inputs your flake file depends on, including things like nixpkgs, System Manager itself, and anything else you might import. Instead of pulling the latest version every time you build, the lock file ensures that the same inputs are used consistently across machines and over time. This makes your configuration reproducible, stable, and rollback-friendly. When you do want to update to new versions, you run a command like `nix flake update`, which refreshes the lock file in a controlled way.

## Setting up your project for remote hosting

As you create your flake.nix and set up any supporting files, you'll want to test it out thoroughly before pushing it up to a remote repo.

For this you have a couple options; one is to test it out on the machine you're currently using. However, we recommend against this, as there might be artifacts on your computer that can interfere with the configuration.

Instead, we recommend starting with a fresh machine. One option is to spin up an EC2 instance on AWS; another is to open up a Virtual Box session on your computer.

!!! Important
    You'll need to ensure you have at least 16GB of disk space on the virtual machine. If you go with 8GB, you're going to run out of space.

After starting with a fresh machine, install Nix, copy over your `flake.nix` and supporting files, and test it out. Once you're ready, make sure your `flake.lock` file is up to date. You can create or update the `flake.lock` file by typing:

```sh
nix flake update
```

And make sure you've pushed it up to the repo. (If you don't do this step, Nix will try to build a `flake.lock`, but will be unable to write it to the same location as the other files, and will error out.)

```sh
nix run 'github:numtide/system-manager' --extra-experimental-features 'nix-command flakes' -- switch --flake git+https://github.com/numtide/system-manager-test#default --sudo
```

### When should you update your `flake.lock` file?

Generally, you only need to update your `flake.lock` file when you want newer versions of your inputs (nixpkgs, etc). Updating isn't necessary for daily use; your configuration will continue to work with the locked versions. But you will want to update your `flake.lock` file in cases such as:

* You want newer package versions (e.g. newer `btop`, etc.)
* You want security patches
* You've added new inputs to your flakes (in which case you'll be required to update `flake.lock`)
* You're preparing a fresh install and decide this is a good time to upgrade everything

### Can't System Manager build `flake.lock` for me?

Yes, but only if the `flake.nix` file is local to your machine. The problem is System Manager will try to write a `flake.lock` file in the same location as the `flake.nix` file, which isn't possible (at this time) with a GitHub repo.



### Ensuring success

In order to ensure System Manager retrieves the correct `.nix` files from your repo, we recommend including either a branch or a tag along with your repo.



## Running System Manager with a remote flake

!!! Tip
    Before you run this command, we recommend that you nevertheless create a folder to run it from, such as `~/.config/system-manager`.

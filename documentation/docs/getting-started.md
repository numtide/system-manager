# Getting Started

If you’ve heard of NixOS, you’ve probably heard that it lets you define your entire system in configuration files and then reproduce that system anywhere with a single command. System Manager brings that same declarative model to any Linux distribution, with no reinstalling, no switching operating systems, and no special prerequisites beyond having Nix installed.

Instead of manually installing packages, editing /etc files, or configuring system services by hand, you describe the desired state of your machine in a small set of Nix files. System Manager reads that configuration, applies it safely, and keeps previous versions so you can roll back at any time. This guide introduces those ideas step by step, helping you gain the benefits of Nix-style reproducibility and consistency on the Linux system you already have.

# System Prerequisites

In order to run System Manager, you need to have:

* Nix installed for all users

* At least 12GB of disk space. (This is important in case you're running small systems, for example, in the cloud.)

* Flakes turned on. (System Manager can work without Flakes, but for this Getting Started guide, we're using Flakes.)

!!! Important
    System Manager does not work with the single-user installation option for Nix.

!!! Important
    At this time, System Manager requires flakes to be enabled.

## How can I tell whether Nix is installed for the whole system or just me?

Simply type

```
which nix
```


If you see it's installed off of your home directory, e.g.:

```
/home/username/.nix-profile/bin/nix
```

Then it's installed just for you. Alternatively, if it's installed for everybody, it will be installed like so:

```
/nix/var/nix/profiles/default/bin/nix
```

# Initializing Your System

To get started with System Manager, you can run our init subcommand, which will create an initial set of files in the `~/.config/system-manager` folder. 

For this first step to work, you **must** enable experimental features in the nix.conf file. (Simply adding the flags to the nix command isn't enough in this step. Afterwards you can remove the setting from your nix.conf file.)

In the shell prompt, use your favorite editor with sudo to open the following file:

```
vi /etc/nix/nix.conf
```

Add the following line if it isn't already present:

```
experimental-features = nix-command flakes
```

Save the file and exit. Next, enter the following:

```
nix run 'github:numtide/system-manager' -- init
```

(Remember, the double dash -- signifies that any options following it are passed to the following command, in this case System Manager, rather than to the main command, `nix`).

Then answer yes to the four questions.

After running the command you will have the following files in your `~/.config/system-manager` folder:

* `flake.nix` -- A flake entrypoint that loads the system.nix file
* `system.nix` -- The declarative file that describes what your system should look like.

!!! Tip
    Because this is your first time running System Manager, Nix will download and build several files, which might take some time. This only happens once, and in the future, System Manager will run very quickly.

!!! Note
    If you activate flakes through the command-line, but not through your /etc/nix/nix.conf file, then System Manager won't create the initial flake.nix file for you. In that case, you can manually create it and paste in the code we provide below, or activate the experimental features (nix-command and flakes) in /etc/nix/nix.conf, and then re-run the System Manager init command.

Here are the contents of the files that were created:

## flake.nix

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
            ./system.nix 
        ];

        # Optionally specify extraSpecialArgs and overlays
      };
    };
}
```

## system.nix

```nix
{ lib, pkgs, ... }:
{
  config = {
    nixpkgs.hostPlatform = "x86_64-linux";

    # Enable and configure services
    services = {
      # nginx.enable = true;
    };

    environment = {
      # Packages that should be installed on a system
      systemPackages = [
        # pkgs.hello
      ];

      # Add directories and files to `/etc` and set their permissions
      etc = {
        # with_ownership = {
        #   text = ''
        #     This is just a test!
        #   '';
        #   mode = "0755";
        #   uid = 5;
        #   gid = 6;
        # };
        #
        # with_ownership2 = {
        #   text = ''
        #     This is just a test!
        #   '';
        #   mode = "0755";
        #   user = "nobody";
        #   group = "users";
        # };
      };
    };

    # Enable and configure systemd services
    systemd.services = { };

    # Configure systemd tmpfile settings
    systemd.tmpfiles = {
      # rules = [
      #   "D /var/tmp/system-manager 0755 root root -"
      # ];
      #
      # settings.sample = {
      #   "/var/tmp/sample".d = {
      #     mode = "0755";
      #   };
      # };
    };
  };
}
```

# Example: Installing/Uninstalling Apps

First, let's build a configuration file that installs or uninstalls apps.

!!! Tip
    The idea is that the configuration file describes what the system should look like. Keep that in mind, as opposed to thinking that the configuration file "installs software" or "uninstalls software."

To get started, we'll create another .nix file that will install a single app. Then we'll run System Manager, and verify it's installed.

Then to demonstrate what System Manager can do, we'll add another line to the configuration file with another app; run System Manager again, and again verify its installation.

Then after that we'll remove one of the apps from the configuration file, run System Manager, and verify that the app is no longer installed.

This will fully demonstrate the declarative nature of these configuration files.

First, in the ~/.config/system-manager folder, create a file apps.nix and place the following in it:

```nix
{ pkgs, ... }:
{
  nixpkgs.hostPlatform = "x86_64-linux";
  
  environment.systemPackages = with pkgs; [
    tldr
  ];
}
```

This configuration states that the system being configured should have the `tldr` app present, and if isn't, System Manager will install it. (Notice how we phrased that! We didn't just say this file installs the app. With .nix files, it's important to get into the mindset that they state what the system should look like.)

Now add the file to the modules list in flake.nix by replacing this line:

```nix
        modules = [ ./system.nix ];
```

with

```nix
        modules = [
            ./system.nix
            ./apps.nix
        ];
```

Note: By default, system.nix includes starter code and some commented out examples, and nothing else. So you can leave it in the list; in its original state, it doesn't do anything.

Next, we'll run System Manager.


```
sudo env PATH="$PATH" nix run 'github:numtide/system-manager' -- switch --flake .
```

After a short moment, the `tldr` app should be installed on your system.

!!! Tip
    The first time you install software with System Manager, it adds a path to your $PATH variable by creating an entry in /etc/profile.d/. This won't take effect until you log out and back in; or you can source the file like so: `source /etc/profile.d/system-manager-path.sh` After that, you should find the tldr program: `which tldr` should yield `/run/system-manager/sw/bin//tldr`.

Now to demonstrate the declarative feature of System Manager, let's add another app to the list. Here's a fun app called cowsay. Add a single line "cowsay" to the list passed into systemPackages:

```nix
{ pkgs, ... }:
{
  config = {
    nixpkgs.hostPlatform = "x86_64-linux";

    environment.systemPackages = with pkgs; [
      tldr
      cowsay
    ];
  };
}
```

Run System Manager again with the same command as above, and you should now have `cowsay` on your system:


```bash
~/.config/system-manager$ cowsay Hello!
 ________
< Hello! >
 --------
        \   ^__^
         \  (oo)\_______
            (__)\       )\/\
                ||----w |
                ||     ||
~/.config/system-manager$
```

Remember, this is a declarative approach; System Manager did not re-install `tldr`. It looked at the list (tldr, cowsay) and compared it to what is currently installed. It saw that `tldr` is already installed, so it skipped that one. It saw `cowsay` is *not* installed, so it installed it, so that the system matches the configuration file.

Now let's remove `cowsay` from the list of installed software. To do so, simply remove the line (or comment it out):

```nix
{ pkgs, ... }:
{
  config = {
    nixpkgs.hostPlatform = "x86_64-linux";

    environment.systemPackages = with pkgs; [
      tldr
    ];
  };
}
```

Notice this file now looks exactly as it did before adding in cowsay, meaning System Manager the system will now look like it did before adding in `cowsay`. Re-run System Manager and you'll see that `cowsay` is no longer installed.

## Understanding the config attribute set

In the above example, we added attributes to the systemPackages set, which is a part of the environment attribute set, which in turn is part of the config attribute set.

When System Manager runs, with the help of the Nix language, you can have multiple config attribute sets, and System Manager combines them into a single attribute set. This allows you to have different setups in separate files, and simply combine them side by side, only having to add on to this line:

```nix
        modules = [
            ./system.nix
            ./apps.nix
        ];
```

However, you need to be careful. Suppose you have a different set of software you want to install, and you create a flake in another area in the filesystem with that software. It might not work the way you intend.

With that second flake, System Manager will gather up any apps you have in the systemPackages attribute, and compare that to what it has already installed earlier. If the packages installed earlier aren't included, it will remove those packages. (That includes the apps you installed with the "other" flake.)

In other words, you cannot have two separate flakes, one for one set of software, the other for a different set of software, and bounce between those flakes. System Manager will treat the second as requesting to uninstall what it installed earlier.

To make the above work, your best bet is to create a single flake and add in individual files that contain the apps you want to install, and always run from that same location.

# Concepts for people new to Nix

[Not sure we want this here, or at all, but it's a start. I think this will help people who are new to Nix. If we don't want it, I'll move it to my own personal website.]

## Understanding Imperative State vs Declarative State

Imperative state means you change the system by hand, step by step. You run commands like apt install, edit files under /etc with a text editor, toggle systemd services, and make changes as you think of them. You’re telling the computer how to do something:

> "Install this package, then edit this file, then restart this service."

Each action mutates the system in place, and over time the machine can drift into a state that’s hard to reproduce.

(To "mutate" something simply means to change it in place. When a system mutates, its files, settings, or state are altered directly, step by step, rather than being reconstructed from a clean, known description.)

Declarative state, on the other hand, means you don’t tell the system how to do the steps — you tell it what you want the final system to look like, and the tool (System Manager, NixOS, Home Manager, etc.) figures out the steps automatically.

> "This machine should have these packages, these /etc files, and these services enabled."
When you activate that configuration, the tool builds the desired end state and applies it in a predictable, repeatable way.

Here's A simple analogy:

Imperative is like writing a recipe with every individual action: "Chop onions. Heat pan. Add oil..."

Declarative is like saying, "I want a finished lasagna," and the system knows how to assemble it reliably every time.

Declarative state avoids drift, keeps everything versioned and reproducible, and makes rollback simple. Imperative state is flexible and quick, but much harder to track or repeat.

> Traditional programming languages are typically imperative in nature. 

If you're familiar with coding, a language like JavaScript is imperative in that you describe everything in a step by step fashion. A language like HTML is declarative in that you simply state what the web page should look like, without saying how to do it.

## A note about objects in your `.nix` files

Nix gives you significant flexibility in creating your objects that you use inside a `.nix` file.

For example, you could have a `config` object that looks like this:

```
config = {
  nixpkgs = {
    hostPlatform = "x86_64-linux";
  }
}
```

This declares an object stored as `config` with a single member called `nixpkgs`; that `nixpkgs` member then has a single member called `hostPlatform`, holding the string literal `"x86_64-linux"`.

But Nix allows great flexilibyt in how you declare such objects. Consider the following:

```nix
  config = {
    nixpkgs.hostPlatform = "x86_64-linux";
  }
```
This creates the exact same object. Nix allows you to string together members with a dot between them, and it will construct the inner object accordingly.

!!! Note
    In the examples throughout this and other guides here, we use a mixture of the above syntax.


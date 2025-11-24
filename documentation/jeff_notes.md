Jeff's private notes -- to be deleted later

Also possibly include:
* Trusted users
* Trusted keys

Try this initially:
allowed-users = *
trusted-users = root

Then they need to add these same options to their flake


Calling init:

sudo env PATH=/home/ubuntu/.nix-profile/bin:/nix/var/nix/profiles/default/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:/usr/games:/usr/local/games:/snap/bin nix run 'github:numtide/syste
m-manager' -- init delete_me/

sudo env PATH=/home/ubuntu/.nix-profile/bin:/nix/var/nix/profiles/default/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:/usr/games:/usr/local/games:/snap/bin nix run 'github:numtide/syste
m-manager' -- init delete_me2/ --no-flake


Adding further configs:

{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    system-manager = {
      url = "github:numtide/system-manager";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, system-manager }: {
    nix.settings.experimental-features = "nix-command flakes";
    systemConfigs.default = system-manager.lib.makeSystemConfig {
      modules = [
        {
            nix.settings.experimental-features = "nix-command flakes";
        }
        ./modules
      ];
    };
  };
}






# Configuration

## Regarding the /etc/nix/nix.conf file and how to work with it

* Option A: Remove your current and let system-manager manage it

  * Then where to put your own custom configurations such as experimental features

* Option B: Keep your own non-sym-link file, and ignore the message from system-manager

## Ways to invoke system-manager

* sudo
* running from root (make sure nix is in the path)



## Directory structure

### Where to put your configuration files
* Option A: A single folder for all your configuration (recommended)
* Option B: A separate folder for each use case

### How to arrange your files

* Option A: A single flake.nix file
* Option B: A reusable flake.nix file with a modules folder (Recommended when using option A above)


### Managing files with Git/GitHub



# Difference between system-manager and NixOS


=== MORE ===

Managing Services (systemd)

This is a big one because systemd is a huge part of system configuration.

Topics:

How System Manager writes service files under /etc/systemd/system

How to create a systemd service in a System Manager module

How overrides work (systemd.services.<name>.serviceConfig)

How to manage timers

How to restart/reload services after switching

This section is incredibly useful for real-world users



Managing Software

Go beyond “installing packages”:

Installing packages system-wide

Removing packages

Adding channels or overlays (if relevant)

Overriding package versions

Pinning nixpkgs in flakes

Using nixpkgs.config in a System Manager module



Dealing With Imperative State

This often confuses new users.

Topics:

Files created outside System Manager

What happens if you hand-edit /etc files

How to make System Manager “own” or “stop owning” a file

Reconciling drift (how to know what changed)



Working With /etc Files Declaratively

Break into subtopics:

Files

Directories

Templates

Permissions (important!)

Symlinks vs. managed files



Troubleshooting Guide

This is extremely helpful for new users.

Common problems:

system-manager: permission denied

nix: command not found when running as root

“Managed file conflict” warnings

My service didn’t start automatically

"Executable 'treefmt' not found" (dev env issues)

/etc/nix/nix.conf conflicts

systemd not picking up changes

Suggested commands to debug:

systemctl status

journalctl -u <service>

nix-store --verify

Profile locations (nix profile list)




User Management

Important for servers:

Creating system users

Managing groups

Setting home directory options

SSH authorized keys management



Security-Related Configuration

These topics are extremely popular for production users:

/etc/sudoers.d

firewall configuration (iptables, nftables)

locking down SSH

Managing DNS and hostnames

Environment variables under /etc/profile.d



Filesystem & Mounts

Not as essential as NixOS, but still useful:

Managing fstab entries

Bind mounts

tmpfiles rules /etc/tmpfiles.d



Networking

People will definitely want:

Configuring /etc/hosts

DHCP/static network config (if applicable)

Managing resolv.conf

VPN config (WireGuard/OpenVPN)



Library of Common Recipes

This section sells a tool like System Manager.

Useful examples:

Install and configure nginx

Create a systemd service that runs a python script

Manage sshd_config

Add custom firewall rules

Deploy multiple services using modules

Create /etc/profile.d/my-env-vars.sh



Best Practices

A dedicated best-practices page gives users confidence.

Could include:

Use flakes for reproducibility

Pin nixpkgs and system-manager versions

Keep configuration modular

Use secrets carefully (and alternatives like sops-nix)

Avoid imperative changes to /etc

Prefer modules over ad-hoc scripts



Migrating from…

Migrating from plain Nix

Migrating from imperative apt/yum-based sysadmin workflows

Migrating from “snowflake server” setups

Migrating from NixOS (or using both at once)



Reference

Keep it clean and short:

Supported module options (link to module descriptions)

Supported file types in /etc

Supported systemd options

Glossary (flake, module, closure, profile, generation, etc.)



A Natural Order

Introduction

Concepts

Installing and running System Manager

Directory structure

/etc files

Installing software

Managing systemd services

User and security management

Networking

Best practices

Troubleshooting

Recipes

FAQ





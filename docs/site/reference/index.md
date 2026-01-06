# Reference

This reference documentation provides detailed information about System Manager's features and configuration options.

## Sections

### [CLI Commands](cli.md)

Command-line usage and all available subcommands: `init`, `switch`, `register`, `build`, `deactivate`, `pre-populate`, and `sudo`. Also covers optional local installation.

### [Configuration](configuration.md)

How to organize your System Manager project: folder structure, file organization, workflows for getting started, managing `/etc/nix/nix.conf`, and running in non-interactive settings.

### [Modules](modules.md)

Writing `.nix` configuration modules: the `flake.nix` structure, managing systemd services, installing packages, creating `/etc` files, and configuring tmpfiles.

### [Remote Flakes](remote-flakes.md)

Hosting your configuration in a Git repository: understanding `flake.lock`, setting up remote hosting, and running System Manager from GitHub.

### [Blueprint](blueprint.md)

Using the Blueprint library with System Manager for a standardized project structure.

### [Examples](examples/index.md)

Complete, working examples:

- [PostgreSQL](examples/postgresql.md) - Database server setup
- [Nginx](examples/nginx.md) - HTTP web server
- [Nginx HTTPS](examples/nginx-https.md) - HTTPS with SSL certificates
- [Custom App](examples/custom-app.md) - Deploying a Bun/TypeScript application

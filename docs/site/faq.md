# FAQ

## General Tips and Best Practices

### 1. Always Test in a VM First

Before applying changes to your production system, test in a safe environment:

```bash
# Build the configuration first to check for errors
nix build .#systemConfigs.default

# For actual VM testing, use a tool like NixOS's VM builder
# or test in a container/virtualized environment
```

### 2. Use Flake Inputs Follows

This ensures consistent nixpkgs versions:

```nix
inputs = {
  nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  system-manager = {
    url = "github:numtide/system-manager";
    inputs.nixpkgs.follows = "nixpkgs";  # Use the same nixpkgs
  };
};
```

By default, each flake input pins its own version of its dependencies, which means you could end up with multiple versions of nixpkgs. The `follows` directive tells Nix to use your nixpkgs instead of the one bundled with system-manager, ensuring consistent package versions across your entire configuration while reducing disk usage and evaluation time.

### 3. Modular Configuration

Split your configuration into multiple files:

```
.
├── flake.nix
└── modules
    ├── default.nix
    ├── services.nix
    ├── packages.nix
    └── users.nix
```

### 4. Check Logs

Always check systemd logs after activation:

```bash
sudo journalctl -u system-manager.target
sudo journalctl -xe
```

### 5. Garbage Collection

Regularly clean up old generations:

```bash
# Remove old system-manager profiles
sudo nix-env --profile /nix/var/nix/profiles/system-manager-profiles --delete-generations old

# Run garbage collection
sudo nix-collect-garbage -d
```

### 6. Rollback

If something goes wrong, you can rollback:

```bash
# List generations
sudo nix-env --profile /nix/var/nix/profiles/system-manager-profiles --list-generations

# Rollback to previous generation
sudo nix-env --profile /nix/var/nix/profiles/system-manager-profiles --rollback

# Activate the previous generation
nix run 'github:numtide/system-manager' -- activate --sudo
```

---

## Troubleshooting

### Service Won't Start

```bash
# Check service status
sudo systemctl status <service-name>

# View detailed logs
sudo journalctl -u <service-name> -n 50

# Check if service file exists
ls -la /etc/systemd/system/<service-name>.service
```

### Package Not Found in PATH

If you just installed System Manager, and installed a package through it, try logging out and logging back in to pick up the path.

```bash
# Check if package is in the profile
ls -la /nix/var/nix/profiles/system-manager-profiles/*/bin/

# Verify the package is in your config
cat /etc/installed-packages.txt

# Check PATH
echo $PATH
```

### Permission Denied

Ensure you're running system-manager with sudo:

```bash
nix run 'github:numtide/system-manager' -- switch --flake . --sudo
```

### Configuration Won't Build

```bash
# Check for syntax errors
nix flake check

# Build without activation
nix build .#systemConfigs.default

# View build logs
nix log /nix/store/<hash>
```

---

## Additional Resources

- [System Manager GitHub Repository](https://github.com/numtide/system-manager)
- [System Manager Documentation](https://github.com/numtide/system-manager/tree/main/docs)
- [NixOS Module Options](https://search.nixos.org/options)
- [Nix Package Search](https://search.nixos.org/packages)
- [PR #266: User Management with Userborn](https://github.com/numtide/system-manager/pull/266)

---
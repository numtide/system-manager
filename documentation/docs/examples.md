# System Manager Examples

[Note: This is a WIP -- I will be updating these samples to be more consistent with the recent samples in the README.]

This document provides practical examples of using system-manager to manage system configurations on any Linux distribution. Each example demonstrates different capabilities and use cases.

## Table of Contents

1. [Example 1: Installing Nginx as a systemd Unit](#example-1-installing-nginx-as-a-systemd-unit)
2. [Example 2: Installing Docker](#example-2-installing-docker)
3. [Example 3: Software Package Management (Emacs and Others)](#example-3-software-package-management-emacs-and-others)
4. [Example 4: User Management with Userborn (PR #266)](#example-4-user-management-with-userborn-pr-266)

---

## Example 1: Installing Nginx as a systemd Unit

This example demonstrates how to install and configure nginx as a systemd service using system-manager.

### flake.nix

```nix
{
  description = "System Manager - Nginx Example";

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
        {
          nixpkgs.hostPlatform = "x86_64-linux";

          # Install nginx package
          environment.systemPackages = with nixpkgs.legacyPackages.x86_64-linux; [
            nginx
          ];

          # Create nginx configuration
          environment.etc."nginx/nginx.conf".text = ''
            user nginx nginx;
            worker_processes auto;
            error_log /var/log/nginx/error.log;
            pid /run/nginx.pid;

            events {
              worker_connections 1024;
            }

            http {
              include       /etc/nginx/mime.types;
              default_type  application/octet-stream;

              log_format main '$remote_addr - $remote_user [$time_local] "$request" '
                              '$status $body_bytes_sent "$http_referer" '
                              '"$http_user_agent" "$http_x_forwarded_for"';

              access_log /var/log/nginx/access.log main;

              sendfile on;
              tcp_nopush on;
              keepalive_timeout 65;

              # Default server
              server {
                listen 80 default_server;
                listen [::]:80 default_server;
                server_name _;
                root /var/www/html;

                location / {
                  index index.html index.htm;
                }

                error_page 404 /404.html;
                location = /404.html {
                }

                error_page 500 502 503 504 /50x.html;
                location = /50x.html {
                }
              }
            }
          '';

          # Create a simple index page
          environment.etc."nginx/html/index.html".text = ''
            <!DOCTYPE html>
            <html>
            <head>
              <title>Welcome to nginx via System Manager!</title>
            </head>
            <body>
              <h1>Success!</h1>
              <p>Nginx is running via system-manager.</p>
            </body>
            </html>
          '';

          # Create systemd service for nginx
          systemd.services.nginx = {
            enable = true;
            description = "The nginx HTTP and reverse proxy server";
            after = [ "network.target" ];
            wantedBy = [ "system-manager.target" ];

            serviceConfig = {
              Type = "forking";
              PIDFile = "/run/nginx.pid";
              ExecStartPre = "${nixpkgs.legacyPackages.x86_64-linux.nginx}/bin/nginx -t";
              ExecStart = "${nixpkgs.legacyPackages.x86_64-linux.nginx}/bin/nginx";
              ExecReload = "/bin/kill -s HUP $MAINPID";
              ExecStop = "/bin/kill -s QUIT $MAINPID";
              PrivateTmp = true;
              Restart = "on-failure";
              RestartSec = "10s";
            };
          };

          # Create required directories
          systemd.tmpfiles.rules = [
            "d /var/log/nginx 0755 nginx nginx -"
            "d /var/www/html 0755 nginx nginx -"
            "L+ /var/www/html/index.html - - - - /etc/nginx/html/index.html"
          ];
        }
      ];
    };
  };
}
```

### Usage

```bash
# Create the group and user
sudo groupadd nginx
sudo useradd -r -s /usr/sbin/nologin -g nginx nginx

# Activate the configuration; make sure you're in the directory with the flake.nix file
sudo nix run 'github:numtide/system-manager' -- switch --flake .

# Check nginx status
sudo systemctl status nginx

# Test the web server
curl http://localhost

# View logs
sudo journalctl -u nginx -f
```

### Notes

- Ensure you have the `nginx` user and group created on your system before activating
- You may need to adjust file paths based on your distribution
- The nginx binary location is pinned from nixpkgs to ensure reproducibility

---

## Example 2: Installing Docker

This example shows how to install Docker and configure it as a systemd service.

### flake.nix

```nix
{
  description = "System Manager - Docker Example";

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
        {
          nixpkgs.hostPlatform = "x86_64-linux";

          # Install Docker and related tools
          environment.systemPackages = with nixpkgs.legacyPackages.x86_64-linux; [
            docker
            docker-compose
            docker-buildx
          ];

          # Docker daemon configuration
          environment.etc."docker/daemon.json".text = ''
            {
              "log-driver": "json-file",
              "log-opts": {
                "max-size": "10m",
                "max-file": "3"
              },
              "storage-driver": "overlay2",
              "storage-opts": [
                "overlay2.override_kernel_check=true"
              ]
            }
          '';

          # Create Docker systemd service
          systemd.services.docker = {
            enable = true;
            description = "Docker Application Container Engine";
            documentation = [ "https://docs.docker.com" ];
            after = [ "network-online.target" "firewalld.service" "containerd.service" ];
            wants = [ "network-online.target" ];
            requires = [ "docker.socket" ];
            wantedBy = [ "system-manager.target" ];

            serviceConfig = {
              Type = "notify";
              ExecStart = "${nixpkgs.legacyPackages.x86_64-linux.docker}/bin/dockerd --host=fd://";
              ExecReload = "/bin/kill -s HUP $MAINPID";
              TimeoutStartSec = 0;
              RestartSec = 2;
              Restart = "always";
              StartLimitBurst = 3;
              StartLimitInterval = "60s";
              
              # Security settings
              LimitNOFILE = 1048576;
              LimitNPROC = "infinity";
              LimitCORE = "infinity";
              TasksMax = "infinity";
              Delegate = "yes";
              KillMode = "process";
              OOMScoreAdjust = -500;
            };
          };

          # Docker socket
          systemd.sockets.docker = {
            enable = true;
            description = "Docker Socket for the API";
            wantedBy = [ "sockets.target" ];
            
            socketConfig = {
              ListenStream = "/var/run/docker.sock";
              SocketMode = "0660";
              SocketUser = "root";
              SocketGroup = "docker";
            };
          };

          # Create necessary directories and setup
          systemd.tmpfiles.rules = [
            "d /var/lib/docker 0710 root root -"
            "d /var/run/docker 0755 root root -"
            "d /etc/docker 0755 root root -"
          ];
        }
      ];
    };
  };
}
```

### Usage

```bash
# Activate the configuration
sudo nix run 'github:numtide/system-manager' -- switch --flake /path/to/this/example

# Check Docker service status
sudo systemctl status docker

# Test Docker
sudo docker run hello-world

# Check Docker version
sudo docker --version

# View Docker logs
sudo journalctl -u docker -f
```

### Notes

- Ensure the `docker` group exists on your system
- Add your user to the docker group: `sudo usermod -aG docker $USER`
- You may need to log out and back in for group changes to take effect
- This example uses the Docker socket for API communication

---

## Example 3: Software Package Management (Emacs and Others)

This example demonstrates installing software packages like emacs and other development tools. It also shows what happens when you remove a package from the configuration.

### flake.nix

```nix
{
  description = "System Manager - Software Package Management";

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
        {
          nixpkgs.hostPlatform = "x86_64-linux";

          # Install various software packages
          environment.systemPackages = with nixpkgs.legacyPackages.x86_64-linux; [
            # Editors
            emacs
            vim
            neovim
            
            # Development tools
            git
            tmux
            htop
            
            # Shell utilities
            ripgrep
            fd
            bat
            exa
            fzf
            
            # Network tools
            curl
            wget
            
            # System tools
            tree
            ncdu
            
            # Programming languages
            python3
            nodejs
            go
          ];

          # Create a configuration file for easy reference
          environment.etc."installed-packages.txt".text = ''
            Installed packages via system-manager:
            
            Editors:
            - emacs
            - vim
            - neovim
            
            Development Tools:
            - git
            - tmux
            - htop
            
            Shell Utilities:
            - ripgrep (rg)
            - fd
            - bat
            - exa
            - fzf
            
            Network Tools:
            - curl
            - wget
            
            System Tools:
            - tree
            - ncdu
            
            Programming Languages:
            - python3
            - nodejs
            - go
            
            These packages are managed by system-manager.
            Check /nix/store for the actual installations.
          '';

          # Create a simple systemd service that uses one of the installed packages
          systemd.services.software-info = {
            enable = true;
            description = "Log installed software information";
            serviceConfig = {
              Type = "oneshot";
              RemainAfterExit = true;
            };
            wantedBy = [ "system-manager.target" ];
            script = ''
              echo "=== System Manager Software Installation Report ===" > /tmp/software-report.txt
              echo "Generated on: $(date)" >> /tmp/software-report.txt
              echo "" >> /tmp/software-report.txt
              
              echo "Emacs version:" >> /tmp/software-report.txt
              ${nixpkgs.legacyPackages.x86_64-linux.emacs}/bin/emacs --version | head -n1 >> /tmp/software-report.txt
              echo "" >> /tmp/software-report.txt
              
              echo "Vim version:" >> /tmp/software-report.txt
              ${nixpkgs.legacyPackages.x86_64-linux.vim}/bin/vim --version | head -n1 >> /tmp/software-report.txt
              echo "" >> /tmp/software-report.txt
              
              echo "Git version:" >> /tmp/software-report.txt
              ${nixpkgs.legacyPackages.x86_64-linux.git}/bin/git --version >> /tmp/software-report.txt
              echo "" >> /tmp/software-report.txt
              
              echo "Python version:" >> /tmp/software-report.txt
              ${nixpkgs.legacyPackages.x86_64-linux.python3}/bin/python3 --version >> /tmp/software-report.txt
              
              echo "Report saved to /tmp/software-report.txt"
              cat /tmp/software-report.txt
            '';
          };
        }
      ];
    };
  };
}
```

### Initial Installation

```bash
# Activate the configuration
sudo nix run 'github:numtide/system-manager' -- switch --flake /path/to/this/example

# Verify packages are available
which emacs
which vim
which git
which python3

# Check the software report
cat /tmp/software-report.txt

# List installed packages
cat /etc/installed-packages.txt
```

### Package Removal Demonstration

Now let's see what happens when we remove packages. Create a modified version of the flake:

#### flake-minimal.nix

```nix
{
  description = "System Manager - Minimal Software Package Management";

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
        {
          nixpkgs.hostPlatform = "x86_64-linux";

          # Reduced package list - removed emacs, neovim, and most other tools
          environment.systemPackages = with nixpkgs.legacyPackages.x86_64-linux; [
            # Keep only essential tools
            vim
            git
            htop
          ];

          environment.etc."installed-packages.txt".text = ''
            Installed packages via system-manager (MINIMAL):
            
            Editors:
            - vim
            
            Development Tools:
            - git
            - htop
            
            Note: Many packages have been removed from the previous configuration.
          '';

          systemd.services.software-info = {
            enable = true;
            description = "Log installed software information";
            serviceConfig = {
              Type = "oneshot";
              RemainAfterExit = true;
            };
            wantedBy = [ "system-manager.target" ];
            script = ''
              echo "=== Minimal Software Installation Report ===" > /tmp/software-report.txt
              echo "Generated on: $(date)" >> /tmp/software-report.txt
              echo "" >> /tmp/software-report.txt
              
              echo "Vim version:" >> /tmp/software-report.txt
              ${nixpkgs.legacyPackages.x86_64-linux.vim}/bin/vim --version | head -n1 >> /tmp/software-report.txt
              echo "" >> /tmp/software-report.txt
              
              echo "Git version:" >> /tmp/software-report.txt
              ${nixpkgs.legacyPackages.x86_64-linux.git}/bin/git --version >> /tmp/software-report.txt
              
              cat /tmp/software-report.txt
            '';
          };
        }
      ];
    };
  };
}
```

### Testing Package Removal

```bash
# First, verify emacs is available with the full configuration
which emacs
# Should output: /nix/store/.../bin/emacs

emacs --version
# Should show emacs version

# Now switch to the minimal configuration
sudo nix run 'github:numtide/system-manager' -- switch --flake /path/to/flake-minimal.nix

# Try to run emacs again
which emacs
# Should output: (nothing or "emacs not found")

# Check if the binary still exists in the nix store
ls -la /nix/store/*emacs*/bin/emacs 2>/dev/null || echo "Emacs removed from active profile"

# The package is no longer in the system PATH
echo $PATH
# You'll notice the emacs store path is no longer included

# View the updated installed packages list
cat /etc/installed-packages.txt
# Will show only vim, git, and htop
```

### What Actually Happens When You Remove a Package?

When you remove a package from your system-manager configuration and re-run it:

1. **The package is removed from the system PATH**: The symbolic links in `/nix/var/nix/profiles/system-manager-profiles/*/bin/` will no longer point to the removed package

2. **The Nix store paths remain**: The actual package files stay in `/nix/store/` until garbage collection

3. **No files are deleted from /nix/store automatically**: System-manager doesn't immediately delete packages to allow rollbacks

4. **The package becomes eligible for garbage collection**: Once it's not referenced by any profile, running `nix-collect-garbage` will remove it

5. **Configuration files are updated**: Any `/etc/` files managed by system-manager are updated to reflect the new state

### Demonstration Script

Here's a complete script to demonstrate the package removal behavior:

```bash
#!/bin/bash

echo "=== System Manager Package Removal Demonstration ==="
echo ""

# Step 1: Apply full configuration
echo "Step 1: Installing full software suite..."
sudo nix run 'github:numtide/system-manager' -- switch --flake /path/to/full/config
echo ""

# Step 2: Verify emacs is available
echo "Step 2: Verifying emacs installation..."
which emacs
emacs --version | head -n 1
echo ""

# Step 3: Save the emacs store path
EMACS_PATH=$(which emacs)
echo "Emacs is currently at: $EMACS_PATH"
echo ""

# Step 4: Apply minimal configuration
echo "Step 3: Switching to minimal configuration (removing emacs)..."
sudo nix run 'github:numtide/system-manager' -- switch --flake /path/to/minimal/config
echo ""

# Step 5: Try to find emacs
echo "Step 4: Checking if emacs is still accessible..."
which emacs 2>/dev/null || echo "✓ Emacs is no longer in PATH"
echo ""

# Step 6: Check if files still exist in nix store
echo "Step 5: Checking if emacs files still exist in nix store..."
if [ -f "$EMACS_PATH" ]; then
    echo "✓ Emacs binary still exists at: $EMACS_PATH"
    echo "  (It will be garbage collected when you run: nix-collect-garbage)"
else
    echo "✗ Emacs binary no longer exists"
fi
echo ""

# Step 7: Show what garbage collection would do
echo "Step 6: Preview what garbage collection would remove..."
nix-store --gc --print-dead | grep emacs | head -n 5
echo "  ... (and possibly more)"
echo ""

echo "=== Summary ==="
echo "When you remove a package from system-manager:"
echo "  1. ✓ It's removed from your PATH"
echo "  2. ✓ New sessions won't have access to it"
echo "  3. ✓ Store files remain until garbage collection"
echo "  4. ✓ You can rollback to previous configurations"
echo "  5. ✓ Running 'nix-collect-garbage' removes unused packages"
echo ""
echo "The software is effectively UNINSTALLED from your system perspective,"
echo "but the files remain for potential rollback until you garbage collect."
```

### Key Takeaways

- **Removing packages from the configuration makes them unavailable** - They won't be in PATH for new shells/sessions
- **The software IS effectively uninstalled** from a user perspective
- **Store files persist for rollback capability** until garbage collection
- **You can always rollback** to previous configurations that had those packages
- **Garbage collection is manual** - run `nix-collect-garbage` to reclaim disk space

---

## Example 4: User Management with Userborn (PR #266)

This example demonstrates how to create and manage users using the userborn feature from PR #266. This is currently a work-in-progress feature but shows the future direction of user management in system-manager.

**Note**: This example is based on PR #266 which is still in draft status. The implementation may change before being merged.

### flake.nix

```nix
{
  description = "System Manager - User Management Example";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    system-manager = {
      url = "github:numtide/system-manager";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    # Userborn for user management (from PR #266)
    # Note: This is a WIP feature - pin to a specific commit for stability
    userborn = {
      url = "github:JulienMalka/userborn/stateful-users";
      # For production, pin to a specific commit:
      # url = "github:JulienMalka/userborn/6e8f0d00e683049ac727b626552d5eba7f3471ff";
    };
  };

  outputs = { self, nixpkgs, system-manager, userborn }: {
    systemConfigs.default = system-manager.lib.makeSystemConfig {
      modules = [
        {
          nixpkgs.hostPlatform = "x86_64-linux";

          # Enable userborn for user management
          services.userborn.enable = true;
          services.userborn.package = userborn.packages.x86_64-linux.default;
          
          # Set stateful users mode
          systemd.services.userborn.environment.USERBORN_STATEFUL = "1";

          # Define users
          users.users = {
            # Create a developer user
            alice = {
              isNormalUser = true;
              description = "Alice Developer";
              home = "/home/alice";
              createHome = true;
              homeMode = "0700";
              
              # Set user shell
              shell = nixpkgs.legacyPackages.x86_64-linux.bash;
              
              # Add to groups
              extraGroups = [ "wheel" "docker" "networkmanager" ];
              
              # Set initial password (will prompt to change on first login)
              # Note: In production, use hashedPasswordFile instead
              # Generate with: mkpasswd -m sha-512
              # Example hash for password "changeme":
              initialHashedPassword = "$6$rounds=656000$YourSalt$HashedPasswordString";
              
              # User-specific packages
              packages = with nixpkgs.legacyPackages.x86_64-linux; [
                vim
                git
                tmux
                htop
              ];
            };

            # Create a service account user
            servicebot = {
              isSystemUser = true;
              description = "Service Bot Account";
              home = "/var/lib/servicebot";
              createHome = true;
              group = "servicebot";
              
              # System users typically use nologin
              shell = "${nixpkgs.legacyPackages.x86_64-linux.shadow}/bin/nologin";
            };

            # Create a web developer user
            webdev = {
              isNormalUser = true;
              description = "Web Developer";
              home = "/home/webdev";
              createHome = true;
              
              shell = nixpkgs.legacyPackages.x86_64-linux.zsh;
              extraGroups = [ "www-data" "developers" ];
              
              # User-specific packages for web development
              packages = with nixpkgs.legacyPackages.x86_64-linux; [
                nodejs
                python3
                go
                docker-compose
              ];
            };
          };

          # Define groups
          users.groups = {
            developers = {
              gid = 3000;
              members = [ "alice" "webdev" ];
            };
            
            servicebot = {
              gid = 3001;
            };
          };

          # Enable required shell programs
          programs.bash.enable = true;
          programs.zsh.enable = true;

          # Create user home directory templates
          systemd.tmpfiles.rules = [
            "d /home/alice/.config 0700 alice alice -"
            "d /home/webdev/.config 0700 webdev webdev -"
            "d /var/lib/servicebot 0750 servicebot servicebot -"
          ];

          # Create a welcome message for new users
          environment.etc."skel/.bash_profile".text = ''
            # Welcome message
            echo "Welcome to this system managed by system-manager!"
            echo "Your user account is managed declaratively."
            echo ""
            
            # Source bashrc if it exists
            if [ -f ~/.bashrc ]; then
              source ~/.bashrc
            fi
          '';

          environment.etc."skel/.bashrc".text = ''
            # Basic bash configuration
            export PATH=$HOME/.local/bin:$PATH
            
            # Aliases
            alias ll='ls -alh'
            alias la='ls -A'
            alias l='ls -CF'
            
            # Prompt
            PS1='\[\033[01;32m\]\u@\h\[\033[00m\]:\[\033[01;34m\]\w\[\033[00m\]\$ '
          '';

          # Activation script to set up user environments
          system.activationScripts.user-setup = {
            text = ''
              echo "Setting up user environments..."
              
              # Copy skeleton files to user homes if they don't exist
              for user_home in /home/alice /home/webdev; do
                if [ -d "$user_home" ]; then
                  for skel_file in /etc/skel/.bash_profile /etc/skel/.bashrc; do
                    target="$user_home/$(basename $skel_file)"
                    if [ ! -f "$target" ]; then
                      cp "$skel_file" "$target"
                      chown $(basename $user_home):$(basename $user_home) "$target"
                    fi
                  done
                fi
              done
              
              echo "User environment setup complete."
            '';
          };
        }
      ];
    };
  };
}
```

### Usage

```bash
# Activate the configuration with user management
sudo nix run 'github:numtide/system-manager' -- switch --flake /path/to/this/example

# Verify users were created
id alice
id webdev
id servicebot

# Check user groups
groups alice
groups webdev

# List all users (filter for our created users)
cat /etc/passwd | grep -E 'alice|webdev|servicebot'

# Check home directories
ls -la /home/alice
ls -la /home/webdev
ls -la /var/lib/servicebot

# Switch to the alice user (requires password)
su - alice

# As alice user, check available packages
which vim
which git
which tmux
```

### Setting Passwords

For production use, you should use `hashedPasswordFile` instead of hardcoded passwords:

```nix
users.users.alice = {
  # ... other config ...
  hashedPasswordFile = "/run/secrets/alice-password";
};
```

Generate a hashed password:

```bash
# Generate a hashed password
mkpasswd -m sha-512

# Or use this one-liner to create a password file
mkpasswd -m sha-512 | sudo tee /run/secrets/alice-password
sudo chmod 600 /run/secrets/alice-password
```

### User Modification Example

To modify a user, simply update the configuration and re-run system-manager:

```nix
# Add alice to more groups
users.users.alice = {
  # ... existing config ...
  extraGroups = [ "wheel" "docker" "networkmanager" "video" "audio" ];
  
  # Add more packages
  packages = with nixpkgs.legacyPackages.x86_64-linux; [
    vim
    git
    tmux
    htop
    # New packages
    ripgrep
    fd
    bat
  ];
};
```

### Removing a User

To remove a user, simply remove their configuration:

```nix
# Before: users.users.alice = { ... };
# After: (remove the alice user block entirely)

users.users = {
  # alice removed
  webdev = {
    # ... webdev config remains ...
  };
  servicebot = {
    # ... servicebot config remains ...
  };
};
```

Then re-activate:

```bash
sudo nix run 'github:numtide/system-manager' -- switch --flake /path/to/this/example

# The user will be removed from /etc/passwd and /etc/shadow
# Note: Home directory may remain and need manual cleanup
```

### Important Notes About PR #266

1. **Work in Progress**: This PR is still in draft status and the API may change
2. **Userborn Integration**: Requires the userborn package for systemd-sysusers integration
3. **Stateful Users**: The example uses `USERBORN_STATEFUL = "1"` for stateful user management
4. **Password Management**: Use `initialPassword` or `initialHashedPassword` for first-time setup, then users can change their passwords
5. **Activation Scripts**: The PR adds support for `system.activationScripts` which allows custom setup logic

### Testing the User Creation

Here's a complete test script:

```bash
#!/bin/bash

echo "=== User Management Test Script ==="
echo ""

# Apply the configuration
echo "Step 1: Creating users..."
sudo nix run 'github:numtide/system-manager' -- switch --flake /path/to/user/example
echo ""

# Test user creation
echo "Step 2: Verifying user 'alice' was created..."
if id alice &>/dev/null; then
    echo "✓ User alice exists"
    echo "  UID: $(id -u alice)"
    echo "  GID: $(id -g alice)"
    echo "  Groups: $(groups alice)"
    echo "  Home: $(eval echo ~alice)"
    echo "  Shell: $(getent passwd alice | cut -d: -f7)"
else
    echo "✗ User alice was not created"
fi
echo ""

# Test system user
echo "Step 3: Verifying system user 'servicebot'..."
if id servicebot &>/dev/null; then
    echo "✓ System user servicebot exists"
    echo "  UID: $(id -u servicebot)"
    echo "  Shell: $(getent passwd servicebot | cut -d: -f7)"
else
    echo "✗ System user servicebot was not created"
fi
echo ""

# Test groups
echo "Step 4: Verifying groups..."
if getent group developers &>/dev/null; then
    echo "✓ Group developers exists"
    echo "  GID: $(getent group developers | cut -d: -f3)"
    echo "  Members: $(getent group developers | cut -d: -f4)"
else
    echo "✗ Group developers was not created"
fi
echo ""

# Test home directories
echo "Step 5: Checking home directories..."
for user in alice webdev; do
    if [ -d "/home/$user" ]; then
        echo "✓ Home directory exists for $user"
        ls -ld "/home/$user"
    else
        echo "✗ Home directory missing for $user"
    fi
done
echo ""

echo "=== Test Complete ==="
```

### Advantages of Declarative User Management

1. **Reproducibility**: User accounts are defined in code
2. **Version Control**: User configurations can be tracked in git
3. **Consistency**: Same user setup across multiple machines
4. **Documentation**: User configuration serves as documentation
5. **Rollback**: Can rollback to previous user configurations

---

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
sudo nix run 'github:numtide/system-manager' -- activate
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
sudo nix run 'github:numtide/system-manager' -- switch --flake .
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
- [System Manager Documentation](https://github.com/numtide/system-manager/tree/main/manual)
- [NixOS Module Options](https://search.nixos.org/options)
- [Nix Package Search](https://search.nixos.org/packages)
- [PR #266: User Management with Userborn](https://github.com/numtide/system-manager/pull/266)

---

## Contributing

If you have additional examples or improvements to these examples, please contribute to the [system-manager repository](https://github.com/numtide/system-manager) or this documentation repository.

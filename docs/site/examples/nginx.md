# Nginx

This example shows how to install and configure Nginx as a web server with HTTP support.

!!! Tip
    This is simply an example to help you learn how to use System Manager. The usual way to install nginx under Nix is to use the [nginx package](https://search.nixos.org/packages?channel=25.11&show=nginx&query=nginx).

## Configuration

Here's a `.nix` file that installs and configures nginx as a system service. Note that this version only supports HTTP and not HTTPS; see [Nginx HTTPS](nginx-https.md) for an example that includes HTTPS.

```nix
{ lib, pkgs, ... }:
{
  config = {
    nixpkgs.hostPlatform = "x86_64-linux";

    # Enable and configure services
    services = {
      nginx.enable = true;
    };

    environment = {
      # Packages that should be installed on a system
      systemPackages = [
        pkgs.hello
        pkgs.mariadb
        pkgs.nginx
      ];

      # Add directories and files to `/etc` and set their permissions
      etc = {
        "nginx/nginx.conf"= {

                user = "root";
                group = "root";
                mode = "0644";

                text = ''
# The user/group is often set to 'nginx' or 'www-data',
# but for a simple root-only demo, we'll keep the default.
# user nginx;
worker_processes auto;

# NGINX looks for modules relative to the install prefix,
# but we explicitly point to the Nix store path to be safe.
error_log /var/log/nginx/error.log;
pid /run/nginx.pid;

events {
    worker_connections 1024;
}

http {
    include             ${pkgs.nginx}/conf/mime.types;
    default_type        application/octet-stream;

    sendfile            on;
    keepalive_timeout   65;

    # Basic default server block
    server {
        listen 80;
        server_name localhost;

        # Point the root directory to a standard location or a Nix store path
        root ${pkgs.nginx}/html;

        location / {
            index index.html;
        }

        # Example log files
        access_log /var/log/nginx/access.log;
        error_log /var/log/nginx/error.log;
    }
}
    '';


        };
      };
    };

    # Enable and configure systemd services
    systemd.services = {
        nginx = {
            enable = true;
            description = "A high performance web server and reverse proxy server";
            wantedBy = [ "system-manager.target" ];
            preStart = ''
                mkdir -p /var/log/nginx
                chown -R root:root /var/log/nginx # Ensure permissions are right for root user
            '';
            serviceConfig = {
                Type = "forking";
                PIDFile = "/run/nginx.pid";

                # The main binary execution command, pointing to the Nix store path
                ExecStart = "${pkgs.nginx}/bin/nginx -c /etc/nginx/nginx.conf";

                # The command to stop the service gracefully
                ExecStop = "${pkgs.nginx}/bin/nginx -s stop";

                # NGINX needs to run as root to bind to port 80/443
                User = "root";
                Group = "root";

                # Restart policy for robustness
                Restart = "on-failure";
            };
        };
    };


  };
}

```

## What this configuration does

1. **Installs Nginx** as a system package
2. **Creates `/etc/nginx/nginx.conf`** with a basic HTTP configuration
3. **Creates a systemd service** that:
   - Creates the log directory on startup
   - Runs Nginx with the custom configuration
   - Restarts on failure
4. **Serves the default Nginx welcome page** on port 80

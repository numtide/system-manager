# Nginx HTTPS

This example shows how to install Nginx with HTTPS support using SSL certificates.

## Configuration

Here's an example that installs nginx with HTTPS. This example shows places where you would copy in your own secure certificate information.

```nix
{ lib, pkgs, ... }:
{
  config = {
    nixpkgs.hostPlatform = "x86_64-linux";

    # Enable and configure services
    # Commenting this out -- apparently this loads a bunch of nginx service files we don't need or want
    #services = {
    #  nginx.enable = true;
    #};

    environment = {
      systemPackages = [
        pkgs.hello
        pkgs.mariadb
        pkgs.nginx
      ];

      # Add SSL certificate files to /etc
      etc = {
        # SSL Certificate
        "ssl/certs/your-domain.crt" = {
          user = "root";
          group = "root";
          mode = "0644";
          # Option 1: Embed the certificate directly
          text = ''
-----BEGIN CERTIFICATE-----
MIIDwzCCAqugAwIBAgIUXbQ2ie2/2pxLH/okEB4KEbVDqjEwDQYJKoZIhvcNAQEL...
-----END CERTIFICATE-----
          '';
          # Option 2: Or reference a file from your repo
          # source = ./certs/your-domain.crt;
        };

        # SSL Private Key
        "ssl/private/your-domain.key" = {
          user = "root";
          group = "root";
          mode = "0600";  # Restrict access to private key!
          # Option 1: Embed the key directly
          text = ''
-----BEGIN PRIVATE KEY-----
MIIEvgIBADANBgkqhkiG9w0BAQEFAASCBKgwggSkAgEAAoIBAQC5gQjZxG7rYPub....
-----END PRIVATE KEY-----
          '';
          # Option 2: Or reference a file from your repo
          # source = ./certs/your-domain.key;
        };

        # Optional: Certificate chain/intermediate certificates
        # For this demo we're using a self-signed cert; for a real
        # one, uncomment below and add your
        "ssl/certs/chain.pem" = {
          user = "root";
          group = "root";
          mode = "0644";
          text = ''
            -----BEGIN CERTIFICATE-----
YOUR_CHAIN_CERTIFICATE_HERE...
            -----END CERTIFICATE-----
          '';
        #};

        # Nginx configuration with HTTPS
        "nginx/nginx.conf" = {
          user = "root";
          group = "root";
          mode = "0644";
          text = ''
worker_processes auto;

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

    # SSL Settings
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_prefer_server_ciphers on;
    ssl_ciphers 'ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256:ECDHE-ECDSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-GCM-SHA384';

    # HTTP Server - Redirect to HTTPS
    server {
        listen 80;
        server_name demo.frecklefacelabs.com www.demo.frecklefacelabs.com;

        # Redirect all HTTP to HTTPS
        return 301 https://$server_name$request_uri;
    }

    # HTTPS Server
    server {
        listen 443 ssl;
        server_name demo.frecklefacelabs.com www.demo.frecklefacelabs.com;

        # SSL Certificate files
        ssl_certificate /etc/ssl/certs/your-domain.crt;
        ssl_certificate_key /etc/ssl/private/your-domain.key;

        # Optional: Certificate chain
        # ssl_trusted_certificate /etc/ssl/certs/chain.pem;

        # Optional: Enable OCSP stapling
        ssl_stapling on;
        ssl_stapling_verify on;

        # Optional: Enable HSTS (HTTP Strict Transport Security)
        add_header Strict-Transport-Security "max-age=31536000; includeSubDomains" always;

        root ${pkgs.nginx}/html;

        location / {
            index index.html;
        }

        access_log /var/log/nginx/access.log;
        error_log /var/log/nginx/error.log;
    }
}
          '';
        };
      };
    };

    systemd.services = {
      nginx = {
        enable = true;
        #description = "A high performance web server and reverse proxy server";
        wantedBy = [ "system-manager.target" ];
        preStart = ''
          mkdir -p /var/log/nginx
          chown -R root:root /var/log/nginx

          # Verify SSL certificate files exist
          if [ ! -f /etc/ssl/certs/your-domain.crt ]; then
            echo "ERROR: SSL certificate not found!"
            exit 1
          fi
          if [ ! -f /etc/ssl/private/your-domain.key ]; then
            echo "ERROR: SSL private key not found!"
            exit 1
          fi
        '';
        serviceConfig = {
          Type = "forking";
          PIDFile = "/run/nginx.pid";
          ExecStart = "${pkgs.nginx}/bin/nginx -c /etc/nginx/nginx.conf";
          ExecStop = "${pkgs.nginx}/bin/nginx -s stop";
          User = "root";
          Group = "root";
          Restart = "on-failure";
        };
      };
    };
  };
}

```

## What this configuration does

1. **Creates SSL certificate files** in `/etc/ssl/`:
   - `/etc/ssl/certs/your-domain.crt` - The public certificate
   - `/etc/ssl/private/your-domain.key` - The private key (with restricted permissions)
   - `/etc/ssl/certs/chain.pem` - Optional intermediate certificates

2. **Configures Nginx for HTTPS**:
   - Redirects HTTP (port 80) to HTTPS
   - Enables TLS 1.2 and 1.3 only
   - Uses strong cipher suites
   - Enables HSTS for security

3. **Creates a systemd service** that:
   - Verifies SSL certificates exist before starting
   - Runs Nginx with the HTTPS configuration

## Security notes

- The private key file uses mode `0600` to restrict access
- TLS 1.0 and 1.1 are disabled for security
- HSTS is enabled to enforce HTTPS
- Replace the placeholder certificate content with your actual certificates

# SELinux

Currently, `system-manager` does not work with SELinux, but we are working on it! For a trail to follow please see [this issue](https://github.com/numtide/system-manager/issues/115).

The Determinate Systems installer automatically creates a policy for nix itself. Please see the [Installation](../installation.md) section for details.
This alone will not be enough for `system-manager` to work, but it's a step in the right direction. As of now, the only way for it to work is to set the policy to permissive.

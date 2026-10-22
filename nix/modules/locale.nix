{
  config,
  lib,
  pkgs,
  ...
}:

{
  config = {
    # NixOS sets both of these on every unit, in
    # nixos/modules/config/i18n.nix and nixos/modules/config/locale.nix.
    # Without them a service built from nixpkgs reads the host's locale and
    # timezone data instead, which the glibc it was linked against cannot
    # necessarily parse.
    #
    # Both are mkDefault, so a configuration that would rather use the host's
    # data (/usr/lib/locale/locale-archive, /usr/share/zoneinfo) can still
    # override them.
    #
    # TZDIR points at the tzdata store path rather than at an /etc/zoneinfo
    # entry, so a configuration with no services still does not touch /etc on
    # the host.
    #
    # LOCALE_ARCHIVE is dropped when i18n.glibcLocales is null, the same shape
    # as the guard NixOS puts around the same assignment. The module system
    # skips null values of systemd.globalEnvironment.
    systemd.globalEnvironment = {
      LOCALE_ARCHIVE = lib.mkDefault (
        if config.i18n.glibcLocales != null then
          "${config.i18n.glibcLocales}/lib/locale/locale-archive"
        else
          null
      );
      TZDIR = lib.mkDefault "${pkgs.tzdata}/share/zoneinfo";
    };
  };
}

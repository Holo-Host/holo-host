/*
this package wraps the official Hivello release debian package.
it currently relies on the nixos config to enable nix-ld with its dependencies installed as libraries.

see `extra-container-hivello.nix` for a usage reference.
*/
{
  inputs,
  system,
  ...
}: let
  pkgs = inputs.nixpkgs-2405.legacyPackages.${system};
  pkgsGbm = inputs.nixpkgs-unstable.legacyPackages.${system};
  inherit
    (pkgs)
    fetchurl
    lib
    stdenv
    ;

  dependencies = with pkgs;
    [
      alsa-lib
      at-spi2-atk
      cairo
      cups
      dbus
      expat
      gdk-pixbuf
      glib
      gtk3
      nss
      nspr
      xorg.libX11
      xorg.libxcb
      xorg.libXcomposite
      xorg.libXdamage
      xorg.libXext
      xorg.libXfixes
      xorg.libXrandr
      xorg.libxkbfile
      xorg.libXScrnSaver
      xorg.libxshmfence
      pango
      pciutils
      stdenv.cc.cc
      systemd
      libdrm
      libxkbcommon
      libGL
      vulkan-loader
      libglvnd

      libgcc.lib
      expat
      nss
      nspr

      mesa

      libgcc.libgcc
    ]
    ++ [
      pkgsGbm.libgbm
    ];
in
  stdenv.mkDerivation {
    pname = "hivello"; # Replace with your package name
    version = "1.3.1"; # Replace with your package version

    src = fetchurl {
      # TODO: get a URL for ${version}
      url = "https://download.hivello.services/linux/deb/x64"; # Replace with the actual URL
      sha256 = "sha256-BjJJKMlA83CbHirCmCdnoqdICkslX5FXlIarzm0Pb8s=";
    };

    nativeBuildInputs = with pkgs; [
      dpkg
    ];

    sourceRoot = ".";
    unpackCmd = ''
      dpkg-deb -x $src .
    '';

    dontConfigure = true;
    dontBuild = true;

    installPhase = ''
      ls -lha
      mkdir -p $out/bin

      cp -r opt usr $out/
      ln -s $out/opt/Hivello/Hivello $out/bin/Hivello
    '';

    meta = with lib; {
      description = "A brief description of your package.";
      platforms = lists.intersectLists platforms.linux platforms.x86_64;

      passthru = {
        inherit dependencies;
      };
    };
  }

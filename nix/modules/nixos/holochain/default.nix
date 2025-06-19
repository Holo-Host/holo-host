# Module to configure a holochain service on a nixos machine.

# blueprint specific first level argument that's referred to as "publisherArgs"
{
  inputs,
  ...
}:

{
  lib,
  config,
  pkgs,
  options,
  ...
}:

let
  cfg = config.holo.holochain;
in
{
  options.holo.holochain = {
    enable = lib.mkOption {
      description = "enable holochain";
      default = true;
    };

    autoStart = lib.mkOption {
      default = true;
    };

    package = lib.mkOption {
      type = lib.types.package;
      default = inputs.holonix_0_5.packages.${pkgs.system}.holochain;
      description = "The Holochain package to use. By default, this is dynamically selected based on the supported versions in `holochainPackage.supportedVersionsConfig`.";
    };

    version = lib.mkOption {
      type = lib.types.str;
      default = "0.5";
      description = "The desired holochain version string (e.g., '0.4', '0.5.1', 'latest').";
    };

    features = lib.mkOption {
      type = with lib.types; nullOr (listOf str);
      default = null;
      description = "A list of features to enable in the holochain package.";
    };

    rust = {
      log = lib.mkOption {
        type = lib.types.str;
        default = "debug";
      };

      backtrace = lib.mkOption {
        type = lib.types.str;
        default = "1";
      };
    };

    wasmLog = lib.mkOption {
      type = lib.types.str;
      default = "info";
      description = "configure wasm log level (zomes)";
    };

    passphraseFile = lib.mkOption {
      type = lib.types.str;
      default = "passphrase.txt";
    };

    adminWebsocketPort = lib.mkOption {
      type = lib.types.int;
      default = 8000;
    };

    adminWebsocketAllowedOrigins = lib.mkOption {
      type = lib.types.str;
      default = "*";
    };

    bootstrapServiceUrl = lib.mkOption {
      type = lib.types.str;
      # Use the Holo-provided default production bootstrap server.
      default = "https://bootstrap.holo.host";
    };

    webrtcTransportPoolSignalUrl = lib.mkOption {
      type = lib.types.str;
      default = "wss://sbd-0.main.infra.holo.host";
    };

    webrtcTransportPoolIceServers = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [
        "stun:stun-0.main.infra.holo.host:443"
        "stun:stun-1.main.infra.holo.host:443"
      ];
    };

    webrtcTransportPool = lib.mkOption {
      type = lib.types.attrs;
      default =
        # Use WebRTC, which is the only option for now.
        {
          type = "webrtc";

          # Use the Holo-provided default production sbd (signal) server.
          # `signal_url` is REQUIRED.
          signal_url = cfg.webrtcTransportPoolSignalUrl;

          # Override the default WebRTC STUN configuration.
          # This is OPTIONAL. If this is not specified, it will default
          # to what you can see here:
          webrtc_config = {
            iceServers = lib.lists.map (url: { urls = [ url ]; }) cfg.webrtcTransportPoolIceServers;
          };
        };
    };

    conductorConfig = lib.mkOption {
      type = lib.types.attrs;
      default = {
        data_root_path = "./holochain_data_root";

        # Configure the keystore to be used.

        # Use an in-process keystore with default database location.
        keystore.type = "lair_server_in_proc";

        # Configure an admin WebSocket interface at a specific port.
        admin_interfaces = [
          {
            driver = {
              type = "websocket";
              port = cfg.adminWebsocketPort;
              allowed_origins = cfg.adminWebsocketAllowedOrigins;
            };
          }
        ];

        # Configure the network.
        network = 
          let
            version = cfg.version;
            
            # Version-aware network configuration:
            # - 0.5 and later: "webrtc_bootstrap" (kitsune2)
            # - 0.4 and earlier: "quic_bootstrap" (legacy kitsune)
            # NB: Defaults to 0.5 behavior if no version is specified
            #
            # Parse version string to get numeric major.minor version
            # Examples: "0.4.1" -> 0.4, "0.5.0" -> 0.5, "0.3" -> 0.3
            parseVersionNumber = versionStr:
              let
                # Split by dots and filter to get only numeric parts
                splitResult = builtins.split "\\." versionStr;
                # Filter out empty strings and separators (keep only numeric strings)
                numericParts = builtins.filter (x: builtins.isString x && builtins.match "[0-9]+" x != null) splitResult;
                major = if builtins.length numericParts >= 1 then builtins.elemAt numericParts 0 else "0";
                minor = if builtins.length numericParts >= 2 then builtins.elemAt numericParts 1 else "0";
                # Convert to float: major.minor
                majorMinorStr = "${major}.${minor}";
                # Parse as float using fromJSON (works for simple decimals)
                versionFloat = builtins.fromJSON majorMinorStr;
              in
                versionFloat;
            
            versionNumber = parseVersionNumber version;
            isLegacyVersion = versionNumber < 0.5;            
            bootstrapField = if isLegacyVersion then "bootstrap_service" else "bootstrap_url";
            networkType = if isLegacyVersion then "quic_bootstrap" else "webrtc_bootstrap";
          in
          {
            network_type = networkType;
            ${bootstrapField} = cfg.bootstrapServiceUrl;
          } 
          // (lib.optionalAttrs (!isLegacyVersion) {
            # For modern versions (>= 0.5), add signal_url at network level
            signal_url = cfg.webrtcTransportPoolSignalUrl;
          })
          // (lib.optionalAttrs isLegacyVersion {
            # For legacy versions (< 0.5), signal_url remains in transport_pool
            transport_pool = [
              {
                type = "webrtc";
                signal_url = cfg.webrtcTransportPoolSignalUrl;
                ice_servers = cfg.webrtcTransportPoolIceServers;
              }
            ];
          });
      };
    };

    conductorConfigOverrides = lib.mkOption {
      type = lib.types.attrs;
      default = { };
    };

    conductorConfigEffective = lib.mkOption {
      type = lib.types.attrs;
      default = lib.attrsets.recursiveUpdate cfg.conductorConfig cfg.conductorConfigOverrides;
    };

    conductorConfigYaml = lib.mkOption {
      type = lib.types.path;
      default = (pkgs.formats.yaml { }).generate "holochain.yml" cfg.conductorConfigEffective;
    };

    user = lib.mkOption {
      type = lib.types.str;
      default = "holochain";
    };
    group = lib.mkOption {
      type = lib.types.str;
      default = "holochain";
    };
  };

  config = 
    let
      # Dynamically set the holochain package based on the version selection logic
      holochainPackage = 
        let
          # NB: We currently embed the version configuration to avoid file accessibility issues in containers
          supportedVersionsConfig = {
            default_version = "0.5";
            supported_versions = [ "0.3" "0.4" "0.5" "latest" ];
            version_mappings = {
              "0.3" = "holonix_0_3";
              "0.4" = "holonix_0_4";
              "0.5" = "holonix_0_5";
              "latest" = "holonix_0_5";
            };
          };

          # Function to select holochain version
          selectHolochainVersion = version: config:
            let
              supportedVersions = config.supported_versions;
              versionMappings = config.version_mappings;
              versionParts = builtins.split "\\." version;

              majorMinor =
                if builtins.length versionParts >= 3
                then "${builtins.elemAt versionParts 0}.${builtins.elemAt versionParts 2}"
                else version;

              mappingKey =
                if builtins.hasAttr version versionMappings
                then version
                else if builtins.hasAttr majorMinor versionMappings
                then majorMinor
                else null;

              holonixInput =
                if mappingKey != null
                then versionMappings.${mappingKey}
                else null;
            in
              if !(builtins.elem version supportedVersions || builtins.elem majorMinor supportedVersions)
              then throw "Unsupported Holochain version '${version}'. Supported versions are: ${builtins.concatStringsSep ", " supportedVersions}"
              else if holonixInput == null
              then throw "No version mapping found for '${version}'. Available mappings: ${builtins.concatStringsSep ", " (builtins.attrNames versionMappings)}"
              else inputs.${holonixInput}.packages.${pkgs.system}.holochain;

          baseHolochainPackage = selectHolochainVersion cfg.version supportedVersionsConfig;
        in
          # Apply features if specified
          if cfg.features != null then
            baseHolochainPackage.override {
              cargoExtraArgs = "--features ${builtins.concatStringsSep "," cfg.features}";
            }
          else
            baseHolochainPackage;
    in
    lib.mkMerge [
      (lib.mkIf cfg.enable {
        # Set the package option using the resolved package
        holo.holochain.package = lib.mkDefault holochainPackage;

        users.groups.${cfg.group} = { };
        users.users.${cfg.user} = {
          isSystemUser = true;
          inherit (cfg) group;
        };

        # Add holochain CLI tools (hc*) to system packages
        # This includes tools like: hc, hc-run-local-services, hc-sandbox, etc.
        # NB: Some tools like hc-run-local-services and hc-sandbox were removed in holonix 0.5
        environment.systemPackages = [
          # Link hc CLI tools from the resolved package (not cfg.package to avoid circular dependency)
          (pkgs.runCommand "holochain-cli-tools" { } ''
            mkdir -p $out/bin
            for bin in ${holochainPackage}/bin/hc*; do
              if [ -f "$bin" ] && [ -x "$bin" ]; then
                ln -s $bin $out/bin/
              fi
            done
          '')
        ];

        systemd.services.holochain = {
          enable = true;

          after = [
            "network.target"
          ];
          wants = [
            "network.target"
          ];
          wantedBy = lib.lists.optional cfg.autoStart "multi-user.target";

          restartIfChanged = true;

          environment = {
            RUST_LOG = "${cfg.rust.log},wasmer_compiler_cranelift=warn";
            RUST_BACKTRACE = cfg.rust.backtrace;
            WASM_LOG = cfg.wasmLog;
          };

          preStart = ''
            passphrase_path="$STATE_DIRECTORY/${cfg.passphraseFile}"
            if [[ ! -e "$passphrase_path" ]]; then
              echo generating new passphrase at "$passphrase_path".
              ${lib.getExe pkgs.pwgen} 64 1 > "$passphrase_path"
            else
              echo using existing passphrase at file "$passphrase_path".
            fi
          '';

          serviceConfig =
            let
              StateDirectory = "holochain";
            in
            {
              User = cfg.user;
              Group = cfg.user;
              # %S is short for StateDirectory
              WorkingDirectory = "%S/${StateDirectory}";
              inherit StateDirectory;
              StateDirectoryMode = "0700";
              Restart = "always";
              RestartSec = 1;
              # The conductor sends a notify signal to systemd when it is ready
              Type = "notify";
              NotifyAccess = "all";
            };

          script = builtins.toString (
            pkgs.writeShellScript "holochain-wrapper" ''
              set -xeE

              ${lib.getExe' holochainPackage "holochain"} \
                --piped \
                --config-path ${cfg.conductorConfigYaml} \
                < "$STATE_DIRECTORY/${cfg.passphraseFile}"
            ''
          );
        };
      })
    ];
}

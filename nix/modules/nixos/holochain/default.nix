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
      description = "The Holochain package to use. By default, this is dynamically selected based on the 'version' and 'versionConfigPath' options.";
    };

    version = lib.mkOption {
      type = with lib.types; nullOr str;
      default = null;
      description = "The desired holochain version string (e.g., '0.4', '0.5.1', 'latest').";
    };

      versionConfigPath = lib.mkOption {
      type = with lib.types; nullOr path;
        default = ../../supported-holochain-versions.json;
      description = "Path to the supported-holochain-versions.json file.";
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
      default = "./passphrase.txt";
    };

    adminWebsocketPort = lib.mkOption {
      type = lib.types.int;
      default = 1234;
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
        network = {
          bootstrap_service = cfg.bootstrapServiceUrl;

          # TODO: See if we can combine this with the version selection logic in other areas...
          # Version-aware network type selection:
          # - 0.4 and earlier: "quic_bootstrap" (legacy kitsune)
          # - 0.5 and later: "webrtc_bootstrap" (kitsune2)
          # Default to 0.5 behavior if no version is specified
          network_type = 
            let
              version = cfg.version or "0.5";
              versionParts = builtins.filter (x: x != "" && builtins.isString x) (builtins.split "\\." version);
              majorVersion = if builtins.length versionParts >= 1 then builtins.elemAt versionParts 0 else "0";
              minorVersion = if builtins.length versionParts >= 2 then builtins.elemAt versionParts 1 else "0";
              majorMinor = "${majorVersion}.${minorVersion}";
              useLegacyNetworking = (majorMinor == "0.3" || majorMinor == "0.4" || version == "0.3" || version == "0.4");
            in
              if useLegacyNetworking then "quic_bootstrap" else "webrtc_bootstrap";

          # Setup a specific network configuration.
          transport_pool = [ cfg.webrtcTransportPool ];
        };
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
      default = "holochain";
    };
    group = lib.mkOption {
      default = "holochain";
    };
  };

  config = lib.mkMerge [
    (lib.mkIf cfg.enable {
      # Dynamically set the package based on the version selection logic
      holo.holochain.package = lib.mkDefault (
        let
          hcVersionsConfig =
            if cfg.versionConfigPath != null && builtins.pathExists cfg.versionConfigPath
            then builtins.fromJSON (builtins.readFile cfg.versionConfigPath)
            else null;

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

        in
          if hcVersionsConfig != null then
            selectHolochainVersion (cfg.version or hcVersionsConfig.default_version) hcVersionsConfig
          else
            # Holochain package fallback if/when the holochain version config is missing.
            inputs.holonix_0_5.packages.${pkgs.system}.holochain
      );

    users.groups.${cfg.group} = { };
    users.users.${cfg.user} = {
      isSystemUser = true;
      inherit (cfg) group;
    };

    # Add holochain CLI tools (hc*) to system packages
    # This includes tools like: hc, hc-run-local-services, hc-sandbox, etc.
    # NB: Some tools like hc-run-local-services and hc-sandbox were removed in holonix 0.5
    environment.systemPackages = [
      # Link hc CLI tools from the holochain package
      (pkgs.runCommand "holochain-cli-tools" { } ''
        mkdir -p $out/bin
        for bin in ${cfg.package}/bin/hc*; do
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
        if [[ ! -e "${cfg.passphraseFile}" ]]; then
          echo generating new passphrase at "${cfg.passphraseFile}".
          ${lib.getExe pkgs.pwgen} 64 1 > "${cfg.passphraseFile}"
        else
          echo using existing passphrase at file ${cfg.passphraseFile}.
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
          Type = "notify"; # The conductor sends a notify signal to systemd when it is ready
          NotifyAccess = "all";
        };

      script = builtins.toString (
        pkgs.writeShellScript "holochain-wrapper" ''
          set -xeE

          ${lib.getExe' cfg.package "holochain"} \
            --piped \
            --config-path ${cfg.conductorConfigYaml} \
            < "${cfg.passphraseFile}"
        ''
      );
    };
    })
    (lib.mkIf (cfg.features != null) {
      holo.holochain.package = lib.mkForce (cfg.package.override {
        cargoExtraArgs = "--features ${builtins.concatStringsSep "," cfg.features}";
      });
    })
  ];
}

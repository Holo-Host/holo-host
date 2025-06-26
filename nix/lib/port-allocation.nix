# Port allocation utils for container services
# NB: The purpose of this lib is to create a shared strategy across all nixos modules to avoid port collisions when running multiple containers

{ lib }:

let
  HOLOCHAIN_HTTP_GW_PORT_DEFAULT = 8090;
  PORT_RANGE = 10000;

  # Generate port offset based on container name hash
  generatePortOffset = containerName:
    let
      hash = builtins.hashString "sha256" containerName;
      chars = lib.stringToCharacters (builtins.substring 0 8 hash);
      # TODO: find a lib to convert hex chars to ints more efficiently
      charToInt = c:
        if c == "0" then 0 else
        if c == "1" then 1 else
        if c == "2" then 2 else
        if c == "3" then 3 else
        if c == "4" then 4 else
        if c == "5" then 5 else
        if c == "6" then 6 else
        if c == "7" then 7 else
        if c == "8" then 8 else
        if c == "9" then 9 else
        if c == "a" then 10 else
        if c == "b" then 11 else
        if c == "c" then 12 else
        if c == "d" then 13 else
        if c == "e" then 14 else
        if c == "f" then 15 else 0;
      hashNum = builtins.foldl' (a: c: a * 16 + charToInt c) 0 chars;
      tHashNum = builtins.typeOf hashNum;
      portOffset = lib.mod hashNum PORT_RANGE;
      tPortOffset = builtins.typeOf portOffset;
    in builtins.trace { hashNum = hashNum; tHashNum = tHashNum; portOffset = portOffset; tPortOffset = tPortOffset; } portOffset;

  # Allocate ports for a container with the given base ports and index
  allocatePorts = { basePorts, containerName, index ? 0, privateNetwork ? false }:
    let
      offset = if privateNetwork then 0 else (generatePortOffset containerName);
    in
      let
        # NB: This forced evaluation is needed due to the use of `builtins.trace` in `generatePortOffset`
        # Foring the evaulaution ensures that the debug info is actually evaluated/printed out
        evaluatedOffset = offset;
      in lib.mapAttrs (name: basePort: basePort + evaluatedOffset) basePorts;

  standardPorts = {
    holochain = {
      adminWebsocket = 8000;
      httpGateway = HOLOCHAIN_HTTP_GW_PORT_DEFAULT;
    };
    nats = {
      client = 4222;
      websocket = 443;
      leafnode = 7422;
    };
  };

  # Test the port allocation functions
  tests = {
    testPortAllocation = {
      testBasicAllocation = 
        let
          ports = allocatePorts {
            basePorts = standardPorts.holochain;
            containerName = "test-container";
            index = 0;
            privateNetwork = false;
          };
          adminPort = ports.adminWebsocket;
          httpPort = ports.httpGateway;
        in
          assert (builtins.isInt adminPort);
          assert (builtins.isInt httpPort);
          true;

      testPrivateNetworkAllocation = 
        let
          ports = allocatePorts {
            basePorts = standardPorts.holochain;
            containerName = "test-container";
            index = 0;
            privateNetwork = true;
          };
        in
          assert (ports.adminWebsocket == standardPorts.holochain.adminWebsocket);
          assert (ports.httpGateway == standardPorts.holochain.httpGateway);
          true;

      testMultipleWorkloads = 
        let
          ports1 = allocatePorts {
            basePorts = standardPorts.holochain;
            containerName = "container1";
            index = 0;
            privateNetwork = false;
          };
          ports2 = allocatePorts {
            basePorts = standardPorts.holochain;
            containerName = "container2";
            index = 1;
            privateNetwork = false;
          };
        in
          assert (ports1.adminWebsocket != ports2.adminWebsocket);
          assert (ports1.httpGateway != ports2.httpGateway);
          true;

      testHexConversionSafety = 
        let
          ports = allocatePorts {
            basePorts = standardPorts.holochain;
            containerName = "test-container-123";
            index = 0;
            privateNetwork = false;
          };
          adminPort = ports.adminWebsocket;
          httpPort = ports.httpGateway;
        in
          assert (builtins.isInt adminPort);
          assert (builtins.isInt httpPort);
          true;
    };
  };

  # Run all tests
  runTests = lib.mapAttrsToList (name: test: 
    lib.mapAttrsToList (testName: testFn: 
      if testFn then 
        "Test ${name}.${testName} passed" 
      else 
        throw "Test ${name}.${testName} failed"
    ) test
  ) tests;

  # Export results
  testResults = builtins.concatLists runTests;

in
{
  inherit HOLOCHAIN_HTTP_GW_PORT_DEFAULT allocatePorts standardPorts tests testResults;
} 
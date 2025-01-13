{
  pkgs,
  flake,
  ...
}:

pkgs.testers.runNixOSTest (_: {
  name = "holo-agent-nixostest-basic";

  nodes.machine =
    _:

    {
      imports = [
        flake.nixosModules.holo-agent
      ];

      holo.agent = {
        enable = true;
        rust = {
          log = "trace";
          backtrace = "trace";
        };
      };
    };

  # takes args which are currently removed by deadnix:
  # { nodes, ... }
  testScript = _: ''
    machine.start()
    # machine.wait_for_unit("holo-agent.service")
    machine.wait_for_unit("default.target")
  '';
})

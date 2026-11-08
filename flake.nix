{
  description = "ScottyLabs website";

  nixConfig = {
    extra-substituters = [ "https://scottylabs.cachix.org" ];
    extra-trusted-public-keys = [
      "scottylabs.cachix.org-1:hajjEX5SLi/Y7yYloiXTt2IOr3towcTGRhMh1vu6Tjg="
    ];
  };

  inputs = {
    scottylabs.url = "git+https://git.cmu.dev/ScottyLabs/kennel";
    nixpkgs.follows = "scottylabs/nixpkgs";
    flake-parts.follows = "scottylabs/flake-parts";
  };

  outputs =
    inputs:
    inputs.flake-parts.lib.mkFlake { inherit inputs; } {
      imports = [ inputs.scottylabs.flakeModules.default ];
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];

      perSystem =
        { helpers, ... }:
        {
          scottylabs = {
            project.name = "scottylabs-org";
            deno = {
              enable = true;
              svelte.enable = true;
              dirs = [ "sites/web" ];
            };
          };

          pre-commit.settings.hooks.oxlint.settings.allow = [
            "max-lines"
            "max-lines-per-function"
          ];

          sites.web = {
            package = helpers.buildDenoTask {
              src = ./sites/web;
              pname = "scottylabs-org-web";
              output = "build";
            };
            customDomain = "scottylabs.org";
          };
        };
    };
}

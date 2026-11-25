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
        let
          # The public site and the portal are one SvelteKit build. The site service serves it, so pages, the portal and the API share an origin.
          web = helpers.buildDenoTask {
            src = ./sites/web;
            pname = "scottylabs-org-web";
            output = "build";
          };
        in
        {
          scottylabs = {
            project.name = "scottylabs-org";

            rust.enable = true;
            postgres.enable = true;
            garage.enable = true;

            deno = {
              enable = true;
              svelte.enable = true;
              dirs = [ "sites/web" ];
            };

            # Keycloak redirects back through the local relay in development.
            # APP_URL is the Vite dev server, which proxies /api and /media to the site service.
            ricochet = {
              enable = true;
              appUrl = "http://localhost:5173";
            };

            processes = {
              site = {
                # Pitchfork starts processes without the dev shell's secrets. Load the profile's secrets the way the shell does, so Keycloak sign-in is on once you have logged in to OpenBao. `secretspec run` alone refuses to start without a token; this starts anyway, with dev sign-in only.
                run = ''
                  if secrets=$(secretspec export -P "''${SECRETSPEC_PROFILE:-dev}" --caller scottylabs-site --reason "start the site service"); then
                    eval "$secrets"
                  else
                    echo "site: starting without this project's secrets, so Keycloak sign-in is off. Log in with 'nix run git+https://git.cmu.dev/ScottyLabs/kennel#login', then restart the site." >&2
                  fi
                  unset secrets
                  # Kennel's environment keeps only log lines from a crate named after the project (scottylabs_org); this crate is `site`.
                  export RUST_LOG=info,site=debug
                  exec cargo run -p site
                '';
                depends = [
                  "postgres-setup"
                  "garage-setup"
                ];
              };
              web.run = "cd sites/web && exec deno task dev --port 5173 --strictPort";
            };

            shell.env = {
              SITE_BIND_ADDR = "127.0.0.1:8080";
              # Lets you sign in as a seeded person without Keycloak. The service refuses it unless APP_URL is a localhost address.
              PORTAL_DEV_LOGIN = "1";
              SEED_DEMO_CONTENT = "1";
            };
          };

          pre-commit.settings.hooks.oxlint.settings.allow = [
            "max-lines"
            "max-lines-per-function"
          ];

          services.site = {
            package = helpers.buildRustService {
              src = ./.;
              pname = "site";
              extraSources = [
                "services/site/migrations"
                "services/site/seed"
              ];
              buildArgs.cargoExtraArgs = "--locked -p site";
            };
            customDomain = "scottylabs.org";
            env.STATIC_DIR = "${web}";
          };
        };
    };
}

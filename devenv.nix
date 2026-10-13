{ inputs, ... }:

{
  imports = [
    inputs.scottylabs.devenvModules.default
  ];

  scottylabs = {
    enable = true;
    project.name = "scottylabs-org";

    deno = {
      enable = true;
      svelte.enable = true;
      dirs = [ "sites/web" ];
    };
    secrets.enable = true;

    kennel.sites.web.customDomain = "scottylabs.org";
  };

  git-hooks.hooks.oxlint.settings.allow = [
    "max-lines"
    "max-lines-per-function"
  ];
}

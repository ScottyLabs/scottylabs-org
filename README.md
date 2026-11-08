# ScottyLabs.org!

The official landing page for ScottyLabs.

## Getting started

Install [Lix](https://lix.systems/install/) with the ScottyLabs binary cache:

```bash
curl -sSf -L https://install.lix.systems/lix | sh -s -- install \
  --extra-conf "extra-substituters = https://scottylabs.cachix.org" \
  --extra-conf "extra-trusted-public-keys = scottylabs.cachix.org-1:hajjEX5SLi/Y7yYloiXTt2IOr3towcTGRhMh1vu6Tjg="
```

Then log in to OpenBao by following kennel's [local development](https://docs.kennel.scottylabs.org/guides/secrets.html#local-development) guide, and enter the dev shell with `nix develop`.

Run the dev server with hot reloading:

```bash
cd sites/web && deno task dev
```

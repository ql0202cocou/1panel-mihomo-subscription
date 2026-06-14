# 1Panel App Packaging

> **Status: package complete (0.1.5).** The `0.1.5` app package exposes the full
> install form — admin credentials, public origin/path prefix, fetch/cache/proxy
> tuning, and the `SECURE_COOKIES` override — and the compose file passes them all
> to the service. The Environment Variables table below is the authoritative
> reference; the install form, compose, and code stay consistent with it.

The 1Panel app package lives at:

```text
apps/mihomo-subscription
```

It is intended to follow the official 1Panel app package layout while remaining
a personal/local app package.

## Structure

```text
apps/mihomo-subscription/
  data.yml
  README.md
  README_en.md
  logo.png
  0.1.0/            # historical (incomplete; predates the full install form)
    data.yml
    docker-compose.yml
    data/
  0.1.2/            # historical
    data.yml
    docker-compose.yml
    data/
  0.1.5/            # current release
    data.yml
    docker-compose.yml
    data/
```

## Local Install Path

Copy the app package directory to the 1Panel host:

```bash
/opt/1panel/resource/apps/local/mihomo-subscription
```

Then open the 1Panel App Store and refresh the app list.

## Environment Variables

This table is the authoritative list. The 1Panel install form
(`apps/mihomo-subscription/<version>/data.yml`), the compose file, and the
service code must stay consistent with it. "In package" marks what the current
package already exposes.

| Variable | Source | Default | In package | Purpose |
|----------|--------|---------|------------|---------|
| `PANEL_APP_PORT_HTTP` | Install form | `8080` | Yes | Host web port mapping |
| `RUST_LOG` | Install form | `info` | Yes | Log level |
| `ADMIN_USERNAME` | Install form | — (required) | Yes | Management login account |
| `ADMIN_PASSWORD` | Install form | — (required) | Yes | Management login password |
| `PUBLIC_BASE_URL` | Install form | — (required) | Yes | Externally reachable origin for generated links |
| `PUBLIC_PATH_PREFIX` | Install form (optional) | random | Yes | Seed for the public path prefix; runtime value lives in `app_settings` and is resettable (see `data-model.md`). Empty/blank is ignored and a random prefix is generated |
| `FETCH_TIMEOUT_SECONDS` | Install form | `15` | Yes | Provider fetch total timeout |
| `FETCH_USER_AGENT` | Env (optional) | `clash.meta/1.0` | No | `User-Agent` for provider fetches. Many airport panels gate the subscription on a Clash-family UA and return `403`/`401` to unknown clients; the default matches the common `/clash/i` check and signals Meta support. Override only for panels that require a specific client UA (e.g. Shadowrocket/Stash) |
| `MAX_SUBSCRIPTION_SIZE_MB` | Install form | `8` | Yes | Provider response size limit |
| `CACHE_TTL_MINUTES` | Install form | `15` | Yes | Generated YAML cache TTL (see `security-design.md`) |
| `TRUSTED_PROXY_HOPS` | Install form | `1` | Yes | Reverse proxy hops to trust when deriving the client IP (see `security-design.md`) |
| `SECURE_COOKIES` | Install form (optional) | `auto` (infer from `https://` `PUBLIC_BASE_URL`) | Yes | Force the `Secure` session-cookie attribute. The install form offers `auto`/`true`/`false`; `auto` (and any unrecognized value) falls back to inference. Set `true` when serving over HTTPS through a TLS-terminating reverse proxy (where the app itself speaks plain HTTP); the service logs a warning when cookies end up without `Secure` |
| `PORT` | Compose (fixed) | `8080` | Yes | Container listen port |
| `DATA_DIR` | Compose (fixed) | `/data` | Yes | SQLite data directory |

`PUBLIC_BASE_URL` should only store the externally reachable origin. The random
`PUBLIC_PATH_PREFIX` is prepended before the tokenized subscription endpoint
when generating permanent links:

```text
https://sub.example.com/<public-path-prefix>/api/sub/<token>
```

## Validation Checklist

This checklist describes the package state required for a release. The `0.1.5`
package satisfies every item below.

- Root `data.yml` contains app metadata.
- Version `data.yml` contains `additionalProperties.formFields`.
- Version `data.yml` exposes `ADMIN_USERNAME` and `ADMIN_PASSWORD` install
  fields for the management login.
- Version `data.yml` exposes the remaining install parameters from the
  Environment Variables table above (`PUBLIC_BASE_URL`, `PUBLIC_PATH_PREFIX`,
  `FETCH_TIMEOUT_SECONDS`, `MAX_SUBSCRIPTION_SIZE_MB`, `CACHE_TTL_MINUTES`,
  `TRUSTED_PROXY_HOPS`, `SECURE_COOKIES`).
- `docker-compose.yml` uses `${CONTAINER_NAME}`.
- `docker-compose.yml` passes every install-form variable as an environment
  variable (admin credentials, public origin/prefix, fetch/cache/proxy tuning,
  and `SECURE_COOKIES`).
- Web port form field uses `PANEL_APP_PORT_HTTP`.
- The service is attached to the external `1panel-network`.
- Persistent data is mounted from `./data`.
- The image reference matches the published Docker Hub image
  (`quinlanhoo/mihomo-subscription:<version>`, multi-arch amd64+arm64, pulled by
  the 1Panel host at install; see `docs/release.md` for the offline local-build
  fallback).
- `logo.png` exists (currently a generated placeholder; replace with a real
  design before public distribution).

## Login Configuration

The management Web UI must require login before users can view or change
subscription configuration.

Configure the credentials through the 1Panel app install form and pass them into
the service through compose environment variables:

```yaml
environment:
  - ADMIN_USERNAME=${ADMIN_USERNAME}
  - ADMIN_PASSWORD=${ADMIN_PASSWORD}
```

These credentials protect only the management UI and management APIs. Generated
subscription links remain public but must still require the random public path
prefix and per-profile token.

## Reverse Proxy Host Header

The management API verifies the `Origin` header against the request `Host` on
state-changing requests (CSRF defense in depth). The reverse proxy in front of
the app must therefore preserve the original Host — for nginx/OpenResty:

```nginx
proxy_set_header Host $host;
```

If the proxy rewrites `Host` to the backend address, browser `Origin` and
`Host` will disagree and every login/POST/PUT/DELETE returns `403`.

# 1Panel App Packaging

> **Status: package update pending.** The service is implemented, but the app
> package still only exposes the `PANEL_APP_PORT_HTTP` and `RUST_LOG` install
> fields. The authentication and conversion parameters in the Environment
> Variables table below must be added to the package before the MVP release; the
> checklist items marked **(pending)** track this.

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
  0.1.0/
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
| `ADMIN_USERNAME` | Install form | — (required) | No | Management login account |
| `ADMIN_PASSWORD` | Install form | — (required) | No | Management login password |
| `PUBLIC_BASE_URL` | Install form | — (required) | No | Externally reachable origin for generated links |
| `PUBLIC_PATH_PREFIX` | Install form (optional) | random | No | Seed for the public path prefix; runtime value lives in `app_settings` and is resettable (see `data-model.md`) |
| `FETCH_TIMEOUT_SECONDS` | Install form | `15` | No | Provider fetch total timeout |
| `MAX_SUBSCRIPTION_SIZE_MB` | Install form | `8` | No | Provider response size limit |
| `CACHE_TTL_MINUTES` | Install form | `15` | No | Generated YAML cache TTL (see `security-design.md`) |
| `TRUSTED_PROXY_HOPS` | Install form | `1` | No | Reverse proxy hops to trust when deriving the client IP (see `security-design.md`) |
| `SECURE_COOKIES` | Install form (optional) | inferred from `https://` `PUBLIC_BASE_URL` | No | Force the `Secure` session-cookie attribute. Set `true` when serving over HTTPS through a TLS-terminating reverse proxy (where the app itself speaks plain HTTP); the service logs a warning when cookies end up without `Secure` |
| `PORT` | Compose (fixed) | `8080` | Yes | Container listen port |
| `DATA_DIR` | Compose (fixed) | `/data` | Yes | SQLite data directory |

`PUBLIC_BASE_URL` should only store the externally reachable origin. The random
`PUBLIC_PATH_PREFIX` is prepended before the tokenized subscription endpoint
when generating permanent links:

```text
https://sub.example.com/<public-path-prefix>/api/sub/<token>
```

## Validation Checklist

This checklist describes the package state required before the MVP release.
Items marked **(pending)** are not yet satisfied by the current package.

- Root `data.yml` contains app metadata.
- Version `data.yml` contains `additionalProperties.formFields`.
- **(pending)** Version `data.yml` exposes `ADMIN_USERNAME` and
  `ADMIN_PASSWORD` install fields for the management login.
- **(pending)** Version `data.yml` exposes the remaining install parameters
  from the Environment Variables table above
  (`PUBLIC_BASE_URL`, `PUBLIC_PATH_PREFIX`, `FETCH_TIMEOUT_SECONDS`,
  `MAX_SUBSCRIPTION_SIZE_MB`, `CACHE_TTL_MINUTES`, `SECURE_COOKIES`).
- `docker-compose.yml` uses `${CONTAINER_NAME}`.
- **(pending)** `docker-compose.yml` passes `ADMIN_USERNAME` and
  `ADMIN_PASSWORD` as environment variables.
- Web port form field uses `PANEL_APP_PORT_HTTP`.
- The service is attached to the external `1panel-network`.
- Persistent data is mounted from `./data`.
- The image reference matches a locally built image tag
  (`mihomo-subscription:<version>`, built on the 1Panel host before install).
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

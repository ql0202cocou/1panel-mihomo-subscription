# 1Panel App Packaging

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

## Validation Checklist

- Root `data.yml` contains app metadata.
- Version `data.yml` contains `additionalProperties.formFields`.
- Version `data.yml` exposes `ADMIN_USERNAME` and `ADMIN_PASSWORD` install
  fields for the management login.
- `docker-compose.yml` uses `${CONTAINER_NAME}`.
- `docker-compose.yml` passes `ADMIN_USERNAME` and `ADMIN_PASSWORD` as
  environment variables.
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

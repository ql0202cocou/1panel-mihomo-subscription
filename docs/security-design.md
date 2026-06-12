# Security Design

This document defines the security direction for Mihomo Subscription Manager.
The project is intended to be self-hosted on 1Panel, but it should still be safe
by default and avoid exposing sensitive subscription data.

## Security Goals

- Do not expose original provider subscription URLs.
- Do not allow the service to be used as an internal network scanner.
- Do not make permanent subscription links easy to enumerate.
- Protect all management APIs and the Web UI.
- Keep error messages and logs free of subscription secrets.
- Keep the design practical for a personal/self-hosted 1Panel application.

## Trust Boundaries

The system has three main trust boundaries:

```text
Admin Browser
  -> Web UI
  -> Management API

Public Subscription Client
  -> Public subscription endpoint

Backend Service
  -> Remote provider subscription URL
```

Treat these surfaces differently:

- Management APIs require authentication.
- Public subscription endpoints do not require login, but require a random public
  path prefix and a per-profile token.
- Remote provider subscription fetches must be protected against SSRF.

## Public Link Design

Split public subscription links into an app-level random path and a profile-level
token:

```text
PUBLIC_BASE_URL=https://sub.example.com
PUBLIC_PATH_PREFIX=<random-app-path>
profile_token=<random-profile-token>
```

Generated link:

```text
https://sub.example.com/<PUBLIC_PATH_PREFIX>/api/sub/<profile_token>
```

Recommended properties:

- `PUBLIC_BASE_URL` stores only the externally reachable origin.
- `PUBLIC_PATH_PREFIX` should be a random 16-24 character path segment.
- `profile_token` should be generated from at least 32 random bytes.
- Each profile gets its own token.
- Links must not include database IDs or original provider URLs.

Validation rules for public subscription requests:

```text
public path prefix matches
profile token exists
profile enabled = true
```

Return `404 Not Found` for invalid path, invalid token, or disabled profile. Do
not reveal which part failed.

## Token Rotation

Support these reset operations:

- Reset a single profile token.
- Reset the global public path prefix.
- Disable a profile, making its generated link immediately invalid.

Use cases:

- If one generated subscription link leaks, reset that profile token.
- If the public entry path appears to be scanned, reset `PUBLIC_PATH_PREFIX`.
- If a provider subscription changes, keep the public link stable unless the
  user explicitly resets it.

## Admin Authentication

Management surfaces:

- Web UI.
- Profile CRUD APIs.
- Rule CRUD APIs.
- Token reset APIs.
- System settings APIs.

Recommended initial approach:

- Read `ADMIN_USERNAME` from the environment.
- Read `ADMIN_PASSWORD` from the environment.
- Configure both values through the 1Panel app install form and
  `docker-compose.yml` environment variables.
- Refuse to start, or restrict to localhost, if either value is not set.
- Store only a password hash if persistence is needed.
- Use session cookies for Web UI login.
- Set cookies with `HttpOnly` and `SameSite=Lax`.
- Set `Secure` when the deployment uses HTTPS.
- Add basic login failure rate limiting.

Bearer token authentication can be used for early development, but session
cookies are better for the Web UI experience.

Initial login flow:

```text
GET /login
  -> show login page

POST /api/auth/login
  -> validate ADMIN_USERNAME and ADMIN_PASSWORD
  -> create session cookie

POST /api/auth/logout
  -> clear session cookie
```

Only authenticated sessions can access the Web UI and management APIs. Public
subscription links still use `PUBLIC_PATH_PREFIX` plus profile token and do not
require a login session.

## SSRF Protection

User-provided provider subscription URLs are dangerous because the backend fetches
them. Protect every outbound fetch.

Required URL rules:

- Allow only `http` and `https`.
- Reject empty hosts.
- Reject username/password credentials in URLs.
- Reject localhost names such as `localhost`.
- Reject bare IPs in blocked ranges.
- Resolve domains and check the resolved IPs.
- Re-check every redirect target.
- Limit redirects to a small number, such as 3.

Blocked IPv4 ranges:

```text
0.0.0.0/8
10.0.0.0/8
100.64.0.0/10
127.0.0.0/8
169.254.0.0/16
172.16.0.0/12
192.0.0.0/24
192.168.0.0/16
198.18.0.0/15
224.0.0.0/4
240.0.0.0/4
```

Blocked IPv6 ranges:

```text
::/128
::1/128
fc00::/7
fe80::/10
ff00::/8
```

Docker/internal network ranges may also be blocked through configuration.

Outbound request limits:

- Connect timeout: 5-10 seconds.
- Total request timeout: 10-20 seconds.
- Maximum response size: 5-10 MB.
- Maximum redirects: 3.
- Only fetch text/YAML-like content for subscription processing.

## Sensitive Data Handling

Original provider subscription URLs often contain secrets. Treat them as
sensitive.

Rules:

- Do not log complete provider subscription URLs.
- Do not include provider URLs in public subscription output.
- Do not include provider URLs in generated error messages.
- Management APIs should hide or mask provider URLs by default.
- Exported configuration should not include provider URLs unless explicitly
  requested by an authenticated administrator.

Example masking:

```text
https://example.com/api/sub?token=abcdef
https://example.com/api/sub?token=***
```

## Rate Limiting and Abuse Control

Recommended limits:

- Login attempts: limit by IP and account scope.
- Public subscription downloads: limit by token and source IP.
- Provider refresh operations: limit per profile.
- Manual generate/refresh API: authenticated and rate limited.

For the first self-hosted version, in-memory rate limits are acceptable. If the
app later becomes multi-instance, move limits to a shared store.

## Cache and Refresh Strategy

Avoid fetching provider subscriptions on every public download request.

Suggested behavior:

```text
GET /<public-path>/api/sub/<token>
  -> if fresh generated cache exists, return it
  -> if cache is missing or stale, refresh and return generated YAML
```

Cache recommendations:

- Cache generated Mihomo YAML, not excessive intermediate data.
- Use a configurable TTL, such as 5-30 minutes.
- Store cache by profile and content hash.
- Allow authenticated manual refresh.
- If refresh fails and stale cache exists, optionally return stale cache with a
  logged warning.

## Error Handling

Public subscription endpoint:

- Return `404 Not Found` for invalid path, invalid token, or disabled profile.
- Return generic errors without provider URLs.
- Do not reveal whether a token exists.

Management API:

- Return useful validation errors.
- Do not include provider subscription secrets.
- Log internal details only after masking sensitive fields.

## Security Checklist

- Management APIs require authentication.
- Admin username and password are configured through 1Panel compose environment
  variables.
- Public links use both `PUBLIC_PATH_PREFIX` and profile token.
- Tokens are generated with cryptographically secure randomness.
- Token reset is supported per profile.
- Public path prefix reset is supported globally.
- URL scheme is limited to `http` and `https`.
- DNS resolution results are checked against blocked IP ranges.
- Redirect targets are checked before following.
- Request timeout, redirect limit, and response size limit are enforced.
- Provider URLs are masked in logs and responses.
- Public endpoint returns generic `404` for invalid access.
- Cache prevents public requests from repeatedly fetching provider URLs.

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

Avoid a timing side channel that distinguishes "wrong path prefix" (no database
lookup) from "right prefix, wrong token" (a lookup runs): always perform the
token lookup regardless of whether the path prefix matched, and compare both
the path prefix and the token in constant time. Otherwise response timing lets
an attacker confirm the path prefix independently, defeating the two-factor
(prefix + token) design.

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
- Compare submitted credentials in constant time to avoid timing side
  channels.
- Use session cookies for Web UI login, with at least 128 bits of CSPRNG
  session-ID entropy.
- Store sessions in memory: they are invalidated on service restart, which is
  acceptable for a single-instance self-hosted app.
- Expire sessions after a bounded idle time (default: 7 days).
- Set cookies with `HttpOnly` and `SameSite=Lax`.
- Set `Secure` when the deployment uses HTTPS.
- Add basic login failure rate limiting.

CORS and CSRF:

- The SPA is served same-origin by the Axum service, so the management API
  needs no CORS at all. Do not enable a permissive CORS layer — the
  prototype's `CorsLayer::permissive()` must be removed when session auth
  lands, otherwise cookie-based auth loses its same-origin protection.
- `SameSite=Lax` blocks cross-site cookie sending on non-GET requests. As
  defense in depth, also verify the `Origin` header on state-changing
  management requests when it is present.

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
- Connect to the validated IP itself (pin it in the HTTP client resolver)
  instead of re-resolving at request time. Validating first and letting the
  client resolve again is vulnerable to DNS rebinding (TOCTOU).
- For IPv6 addresses that embed an IPv4 address (IPv4-mapped, NAT64, 6to4),
  extract the embedded IPv4 address and check it against the IPv4 blocklist.
- Re-check every redirect target with the same rules, including IP pinning.
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
192.0.2.0/24
192.88.99.0/24
192.168.0.0/16
198.18.0.0/15
198.51.100.0/24
203.0.113.0/24
224.0.0.0/4
240.0.0.0/4
```

Blocked IPv6 ranges:

```text
::/128
::1/128
::ffff:0:0/96    # IPv4-mapped: extract embedded IPv4 and re-check
64:ff9b::/96     # NAT64: extract embedded IPv4 and re-check
2002::/16        # 6to4: extract embedded IPv4 and re-check
fc00::/7
fe80::/10
ff00::/8
```

The three IPv4-embedding ranges are classic SSRF bypasses: a URL such as
`http://[::ffff:127.0.0.1]/` is not in any blocked IPv6 range by itself, so
the embedded IPv4 address must be extracted and checked against the IPv4
blocklist.

Docker/internal network ranges may also be blocked through configuration.

Outbound request limits:

- Connect timeout: 5-10 seconds.
- Total request timeout: 10-20 seconds.
- Maximum response size: 5-10 MB, enforced on the streamed byte count while
  reading the body. Do not rely on the `Content-Length` header — it is
  attacker-controlled and can lie.
- Maximum redirects: 3.
- Only fetch text/YAML-like content for subscription processing.

## Untrusted Content Handling

Provider responses are untrusted input even after the URL passes SSRF checks.

- Parse fetched YAML with resource limits. YAML anchors/aliases can amplify a
  size-limited input into unbounded memory ("billion laughs"); cap alias
  expansion and nesting depth, or reject documents that exceed them. Apply the
  same parse limits to admin-submitted node/group YAML, and bound every
  management request body (default 1 MB, reject with `413`) — an authenticated
  admin is not a reason to allow unbounded memory or database growth.
- Sanitize the provider `subscription-userinfo` header before storing or
  echoing it on the public endpoint: accept only a single header value
  matching the expected `key=value; ...` shape, and reject values containing
  control characters (CR/LF) to prevent response header injection.
- Treat provider node names and group names as plain data. The Web UI must
  escape them on render; never interpolate them into HTML or shell commands.

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
- The Web UI never persists provider URLs in browser storage
  (localStorage/sessionStorage); only masked values are ever held client-side.

Example masking:

```text
https://example.com/api/sub?token=abcdef
https://example.com/api/sub?token=***
```

Masking rule (deterministic, applied everywhere): keep the scheme, host, and
path; replace every query parameter value with `***`. The same rule applies
to logs, API responses, and error messages.

## Rate Limiting and Abuse Control

Recommended limits:

- Login attempts: limit by IP and account scope.
- Public subscription downloads: limit by token and source IP.
- Provider refresh operations: limit per profile.
- Manual generate/refresh API: authenticated and rate limited.

For the first self-hosted version, in-memory rate limits are acceptable. If the
app later becomes multi-instance, move limits to a shared store.

### Deriving the Client IP

The service runs behind the 1Panel reverse proxy, so the TCP peer address is
always the proxy, not the client. Rate limits and access logs that key on
"source IP" must therefore derive the client IP correctly:

- Trust `X-Forwarded-For` only from the known reverse proxy, and take the
  rightmost untrusted hop — not the leftmost, which is client-controlled and
  spoofable.
- Make the number of trusted proxy hops configurable
  (`TRUSTED_PROXY_HOPS`, default `1` for the standard 1Panel deployment).
- If the header is absent or malformed, fall back to the TCP peer address.

Getting this wrong collapses all clients into the proxy IP (rate limits become
global) or lets a client forge the header to evade per-IP limits.

## Cache and Refresh Strategy

Avoid fetching provider subscriptions on every public download request.

Suggested behavior:

```text
GET /<public-path>/api/sub/<token>
  -> if fresh generated cache exists, return it
  -> if cache is missing or stale, refresh and return generated YAML
```

Single-flight refresh (required): coalesce concurrent refreshes of the same
profile behind a per-profile lock so a stale-cache stampede cannot fan out
into many simultaneous provider fetches. Concurrent requests for that profile
either await the in-flight refresh or serve the existing stale cache; only one
upstream fetch runs at a time per profile. Without this, one popular expiring
link multiplies into N provider fetches, hammering the provider and amplifying
outbound load.

Cache recommendations:

- Cache generated Mihomo YAML, not excessive intermediate data.
- Use a configurable TTL via `CACHE_TTL_MINUTES` (default 15; see the
  environment variable table in `technical-roadmap.md`).
- Store cache by profile and content hash.
- Allow authenticated manual refresh.
- If refresh fails and stale cache exists, optionally return stale cache with a
  logged warning.

## Error Handling

Public subscription endpoint:

- Return `404 Not Found` for invalid path, invalid token, or disabled profile.
- Return a generic `503` when the request is valid but no cache exists and
  the provider fetch failed; the body must contain no upstream details.
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
- DNS resolution results are checked against blocked IP ranges, and the
  validated IP is pinned for the actual connection (DNS-rebinding safe).
- IPv4-embedding IPv6 addresses (IPv4-mapped, NAT64, 6to4) are unwrapped and
  re-checked against the IPv4 blocklist.
- Redirect targets are checked before following.
- Request timeout, redirect limit, and response size limit are enforced; the
  size limit counts streamed bytes, not `Content-Length`.
- Fetched YAML is parsed with alias-expansion and nesting-depth limits.
- `subscription-userinfo` is format-validated before being stored or echoed.
- Credentials are compared in constant time.
- No permissive CORS layer; the management API is same-origin only, with
  `Origin` verification on state-changing requests as defense in depth.
- Management request bodies are size-bounded (`413` on overflow) and
  admin-submitted YAML uses the same parse limits as provider content.
- Provider URLs are masked in logs and responses.
- Public endpoint returns generic `404` for invalid access, with the token
  lookup always performed and constant-time comparison to avoid timing
  disclosure of the path prefix.
- Cache prevents public requests from repeatedly fetching provider URLs, and a
  per-profile single-flight lock prevents stale-cache refresh stampedes.
- Client IP for rate limiting is derived from a trusted reverse proxy hop, not
  a client-spoofable header.

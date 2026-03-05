# CDN Design: Cloudflare R2 + Worker

**Date:** 2026-03-05
**Status:** Approved

## Overview

A CDN at `cdn.simse.dev` serving static marketing media and versioned product binary downloads. Built on Cloudflare R2 (storage), Workers (routing), and KV (version manifest). CI/CD uploads all assets on release.

## Architecture

```
CI/CD pipeline
  ├── uploads binaries → R2 (releases/{os}/{arch}/{version}/{file})
  ├── uploads media   → R2 (media/{file})
  └── updates KV      → latest:{os}-{arch} = {version}

cdn.simse.dev (Cloudflare Worker)
  ├── GET /media/{file}                    → R2 proxy, long cache
  ├── GET /download/{version}/{os}/{arch}  → R2 proxy, long cache
  └── GET /download/latest/{os}/{arch}     → KV lookup → 301 redirect to versioned URL
```

**New service:** `simse-cdn/` — Cloudflare Worker with one R2 bucket binding and one KV namespace binding. No D1, no auth.

## Wrangler Bindings

```toml
name = "simse-cdn"
compatibility_date = "2025-04-01"

routes = [{ pattern = "cdn.simse.dev", custom_domain = true }]

[[r2_buckets]]
binding = "CDN_BUCKET"
bucket_name = "simse-cdn"

[[kv_namespaces]]
binding = "VERSION_STORE"
id = "<kv-namespace-id>"
```

## Data Layout

### R2 Key Structure

```
media/
  hero.png
  logo.svg
  demo.mp4

releases/
  linux/x64/1.2.3/simse-linux-x64.tar.gz
  darwin/arm64/1.2.3/simse-darwin-arm64.tar.gz
  windows/x64/1.2.3/simse-windows-x64.zip
```

### KV Keys

```
latest:linux-x64     → "1.2.3"
latest:darwin-arm64  → "1.2.3"
latest:windows-x64   → "1.2.3"
```

## URL Routing

| Request | Behavior |
|---------|----------|
| `GET /media/{file}` | Stream from R2, `Cache-Control: public, max-age=31536000, immutable` |
| `GET /download/{version}/{os}/{arch}` | Stream binary from R2, same long cache |
| `GET /download/latest/{os}/{arch}` | KV lookup → 301 redirect to versioned URL |
| `GET /health` | `200 OK` |
| Anything else | `404 Not Found` |

## Response Headers

```
# Media and versioned downloads
Cache-Control: public, max-age=31536000, immutable

# /download/latest/* — never cache, always resolve fresh version
Cache-Control: no-store

# Binary downloads
Content-Disposition: attachment; filename="{filename}"
Content-Type: application/octet-stream

# Media — Content-Type passed through from R2 object metadata
```

## Error Handling

| Condition | Response |
|-----------|----------|
| R2 key not found | `404 Not Found` |
| KV key not found (unknown platform) | `404 Not Found` ("unknown platform") |
| R2 fetch error | `502 Bad Gateway` |

## CI/CD on Release

1. Upload binary to `releases/{os}/{arch}/{version}/{file}` in R2
2. Upload media assets to `media/` in R2 (only on asset changes)
3. `wrangler kv key put latest:{os}-{arch} {version}` for each platform

## Testing

Vitest + `@cloudflare/vitest-pool-workers`. Test cases:
- Media passthrough → correct headers, R2 body streamed
- Versioned download → correct headers, `Content-Disposition` set
- `/download/latest/{os}/{arch}` → 301 to correct versioned URL
- Unknown platform → 404
- Missing R2 key → 404
- Unknown route → 404

# simse-cdn

CDN Cloudflare Worker at `cdn.simse.dev`. Serves media from R2 and versioned binary downloads with latest-version redirect via KV.

## Routes

| Path | Behavior |
|------|----------|
| `GET /media/{file}` | Stream from R2, immutable cache |
| `GET /download/{version}/{os}/{arch}` | Stream binary from R2 |
| `GET /download/latest/{os}/{arch}` | KV lookup, 301 redirect |
| `GET /health` | 200 OK |

## Development

```bash
npm run dev
```

## Test

```bash
npm run test
```

## Lint

```bash
npm run lint
```

# simse-analytics

Centralized analytics and audit service Cloudflare Worker. Consumes queue messages from all 8 services, writing datapoints to Analytics Engine and audit events to D1.

## Development

```bash
npm run dev
```

## Lint

```bash
npm run lint
```

## Migrations

```bash
npm run db:migrate        # local
npm run db:migrate:prod   # remote
```

# simse-payments

Payments service Cloudflare Worker. Manages Stripe subscriptions, credit balances, top-ups, customer sync, billing portal, and webhook handling. Uses D1 for storage.

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

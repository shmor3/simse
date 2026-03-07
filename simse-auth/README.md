# simse-auth

Auth service Cloudflare Worker. Handles user registration, login, 2FA, password reset, session management, team CRUD, invites, member roles, and API key management. Uses D1 for storage and Argon2 for password hashing.

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

#!/usr/bin/env bash
set -euo pipefail

# ============================================================================
# simse deployment setup
#
# Reads secrets from deployment/.env (copy from .env.example).
# Guides you through:
#   1. GitHub Actions secrets (Cloudflare credentials)
#   2. Cloudflare resource creation (D1, KV, R2 for staging + production)
#   3. Secrets Store IDs
#   4. Wrangler secrets for each worker
#   5. Updating wrangler.toml placeholders with real resource IDs
# ============================================================================

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

header() { echo -e "\n${BLUE}${BOLD}══════════════════════════════════════════${NC}"; echo -e "${BLUE}${BOLD}  $1${NC}"; echo -e "${BLUE}${BOLD}══════════════════════════════════════════${NC}\n"; }
step() { echo -e "${CYAN}→${NC} $1"; }
ok() { echo -e "${GREEN}✓${NC} $1"; }
warn() { echo -e "${YELLOW}!${NC} $1"; }
err() { echo -e "${RED}✗${NC} $1"; }
ask() { echo -en "${BOLD}$1${NC} "; }

confirm() {
    ask "$1 [Y/n]"
    read -r reply
    [[ -z "$reply" || "$reply" =~ ^[Yy] ]]
}

# Get a value: first from env var, then prompt. Usage: get_value VAR_NAME "prompt text"
get_value() {
    local var_name="$1" prompt_text="$2"
    local current="${!var_name:-}"
    if [[ -n "$current" ]]; then
        ok "$var_name loaded from .env"
    else
        ask "$prompt_text"
        read -r current
        eval "$var_name='$current'"
    fi
    echo "$current"
}

# Get a secret value (hidden input): first from env var, then prompt
get_secret() {
    local var_name="$1" prompt_text="$2"
    local current="${!var_name:-}"
    if [[ -n "$current" ]]; then
        ok "$var_name loaded from .env"
    else
        ask "$prompt_text"
        read -rs current
        echo
        eval "$var_name='$current'"
    fi
    echo "$current"
}

replace_placeholder() {
    local file="$1" placeholder="$2" value="$3"
    if grep -q "$placeholder" "$file" 2>/dev/null; then
        sed -i "s|$placeholder|$value|g" "$file"
        ok "Updated $file: $placeholder -> ${value:0:12}..."
    fi
}

# ============================================================================
header "Loading .env"
# ============================================================================

ENV_FILE="$REPO_ROOT/deployment/.env"

if [[ -f "$ENV_FILE" ]]; then
    # shellcheck source=/dev/null
    source "$ENV_FILE"
    ok "Loaded deployment/.env"
else
    warn "No deployment/.env found"
    echo "Copy deployment/.env.example to deployment/.env and fill in values."
    echo "Continuing with interactive prompts for all values."
    echo
fi

# ============================================================================
header "Prerequisites Check"
# ============================================================================

MISSING=()
command -v gh >/dev/null 2>&1 || MISSING+=("gh (GitHub CLI)")
command -v wrangler >/dev/null 2>&1 || {
    if command -v bunx >/dev/null 2>&1; then
        WRANGLER="bunx wrangler"
    else
        MISSING+=("wrangler (Cloudflare CLI)")
    fi
}
WRANGLER="${WRANGLER:-wrangler}"

command -v jq >/dev/null 2>&1 || MISSING+=("jq (JSON processor)")

if [[ ${#MISSING[@]} -gt 0 ]]; then
    err "Missing required tools:"
    for tool in "${MISSING[@]}"; do
        echo "    - $tool"
    done
    exit 1
fi

ok "gh CLI found"
ok "wrangler found ($WRANGLER)"
ok "jq found"

step "Checking GitHub CLI auth..."
if ! gh auth status >/dev/null 2>&1; then
    err "Not logged in to GitHub CLI. Run: gh auth login"
    exit 1
fi
ok "GitHub CLI authenticated"

step "Checking Wrangler auth..."
if ! $WRANGLER whoami >/dev/null 2>&1; then
    warn "Not logged in to Wrangler."
    if confirm "Run wrangler login now?"; then
        $WRANGLER login
    else
        exit 1
    fi
fi
ok "Wrangler authenticated"

GH_REPO=$(gh repo view --json nameWithOwner -q .nameWithOwner 2>/dev/null || echo "")
if [[ -z "$GH_REPO" ]]; then
    err "Could not detect GitHub repo. Are you in the right directory?"
    exit 1
fi
ok "Repository: $GH_REPO"

# ============================================================================
header "Step 1: GitHub Actions Secrets"
# ============================================================================

echo "Setting CLOUDFLARE_ACCOUNT_ID and CLOUDFLARE_API_TOKEN on GitHub."
echo

# Account ID — try .env, then wrangler whoami, then prompt
if [[ -z "${CLOUDFLARE_ACCOUNT_ID:-}" ]]; then
    detected=$($WRANGLER whoami --json 2>/dev/null | jq -r '.accounts[0].id // empty' || echo "")
    if [[ -n "$detected" ]]; then
        ok "Detected Account ID from wrangler: $detected"
        CLOUDFLARE_ACCOUNT_ID="$detected"
    fi
fi

CF_ACCOUNT_ID=$(get_value CLOUDFLARE_ACCOUNT_ID "Enter Cloudflare Account ID:")
if [[ -n "$CF_ACCOUNT_ID" ]]; then
    echo "$CF_ACCOUNT_ID" | gh secret set CLOUDFLARE_ACCOUNT_ID --repo "$GH_REPO"
    ok "Set CLOUDFLARE_ACCOUNT_ID on GitHub"
else
    warn "Skipped CLOUDFLARE_ACCOUNT_ID"
fi

CF_API_TOKEN=$(get_secret CLOUDFLARE_API_TOKEN "Enter Cloudflare API Token:")
if [[ -n "$CF_API_TOKEN" ]]; then
    echo "$CF_API_TOKEN" | gh secret set CLOUDFLARE_API_TOKEN --repo "$GH_REPO"
    ok "Set CLOUDFLARE_API_TOKEN on GitHub"
else
    warn "Skipped CLOUDFLARE_API_TOKEN"
fi

# ============================================================================
header "Step 2: Create Cloudflare Resources (Staging)"
# ============================================================================

echo "Creating D1 databases, KV namespace, and R2 bucket for staging."
echo

if confirm "Create staging resources now?"; then

    for db_info in \
        "simse-auth-db-staging:simse-auth/wrangler.toml:PLACEHOLDER_STAGING_AUTH_DB_ID" \
        "simse-payments-db-staging:simse-payments/wrangler.toml:PLACEHOLDER_STAGING_PAYMENTS_DB_ID" \
        "simse-mailer-db-staging:simse-mailer/wrangler.toml:PLACEHOLDER_STAGING_MAILER_DB_ID" \
        "simse-waitlist-staging:simse-landing/wrangler.toml:PLACEHOLDER_STAGING_WAITLIST_DB_ID"
    do
        IFS=':' read -r db_name file placeholder <<< "$db_info"
        step "Creating D1 database: $db_name"
        result=$($WRANGLER d1 create "$db_name" --json 2>/dev/null || echo '{}')
        db_id=$(echo "$result" | jq -r '.uuid // empty')
        if [[ -n "$db_id" ]]; then
            ok "Created $db_name (ID: $db_id)"
            replace_placeholder "$file" "$placeholder" "$db_id"
        else
            warn "Could not create $db_name (may already exist)"
            ask "Enter existing database ID for $db_name (or Enter to skip):"
            read -r db_id
            [[ -n "$db_id" ]] && replace_placeholder "$file" "$placeholder" "$db_id"
        fi
    done

    step "Creating KV namespace: simse-cdn-staging-version-store"
    result=$($WRANGLER kv namespace create "simse-cdn-staging-version-store" --json 2>/dev/null || echo '{}')
    kv_id=$(echo "$result" | jq -r '.id // empty')
    if [[ -n "$kv_id" ]]; then
        ok "Created KV namespace (ID: $kv_id)"
        replace_placeholder "simse-cdn/wrangler.toml" "PLACEHOLDER_STAGING_KV_ID" "$kv_id"
    else
        warn "Could not create KV namespace (may already exist)"
        ask "Enter existing KV namespace ID (or Enter to skip):"
        read -r kv_id
        [[ -n "$kv_id" ]] && replace_placeholder "simse-cdn/wrangler.toml" "PLACEHOLDER_STAGING_KV_ID" "$kv_id"
    fi

    step "Creating R2 bucket: simse-cdn-staging"
    if $WRANGLER r2 bucket create simse-cdn-staging 2>/dev/null; then
        ok "Created R2 bucket: simse-cdn-staging"
    else
        warn "R2 bucket simse-cdn-staging may already exist"
    fi

else
    warn "Skipped staging resource creation"
fi

# ============================================================================
header "Step 3: Create Cloudflare Resources (Production)"
# ============================================================================

echo "Creating D1 databases, KV namespace, and R2 bucket for production."
echo "Note: simse-payments-db and simse_waitlist already have real IDs."
echo

if confirm "Create production resources now?"; then

    for db_info in \
        "simse-auth-db:simse-auth/wrangler.toml:PLACEHOLDER_FILL_AFTER_CREATION" \
        "simse-mailer-db:simse-mailer/wrangler.toml:PLACEHOLDER_FILL_AFTER_CREATION"
    do
        IFS=':' read -r db_name file placeholder <<< "$db_info"
        step "Creating D1 database: $db_name"
        result=$($WRANGLER d1 create "$db_name" --json 2>/dev/null || echo '{}')
        db_id=$(echo "$result" | jq -r '.uuid // empty')
        if [[ -n "$db_id" ]]; then
            ok "Created $db_name (ID: $db_id)"
            replace_placeholder "$file" "$placeholder" "$db_id"
        else
            warn "Could not create $db_name (may already exist)"
            ask "Enter existing database ID for $db_name (or Enter to skip):"
            read -r db_id
            [[ -n "$db_id" ]] && replace_placeholder "$file" "$placeholder" "$db_id"
        fi
    done

    step "Creating KV namespace: simse-cdn-version-store"
    result=$($WRANGLER kv namespace create "simse-cdn-version-store" --json 2>/dev/null || echo '{}')
    kv_id=$(echo "$result" | jq -r '.id // empty')
    if [[ -n "$kv_id" ]]; then
        ok "Created KV namespace (ID: $kv_id)"
        replace_placeholder "simse-cdn/wrangler.toml" "PLACEHOLDER_REPLACE_WITH_KV_ID" "$kv_id"
    else
        warn "Could not create KV namespace (may already exist)"
        ask "Enter existing KV namespace ID (or Enter to skip):"
        read -r kv_id
        [[ -n "$kv_id" ]] && replace_placeholder "simse-cdn/wrangler.toml" "PLACEHOLDER_REPLACE_WITH_KV_ID" "$kv_id"
    fi

    step "Creating R2 bucket: simse-cdn"
    if $WRANGLER r2 bucket create simse-cdn 2>/dev/null; then
        ok "Created R2 bucket: simse-cdn"
    else
        warn "R2 bucket simse-cdn may already exist"
    fi

else
    warn "Skipped production resource creation"
fi

# ============================================================================
header "Step 4: Secrets Store IDs"
# ============================================================================

echo "simse-api and simse-mailer use Cloudflare Secrets Store."
echo "Create a store at: https://dash.cloudflare.com > Workers & Pages > Secrets Store"
echo

prod_store_id=$(get_value SECRETS_STORE_ID_PRODUCTION "Enter production Secrets Store ID (or Enter to skip):")
if [[ -n "$prod_store_id" ]]; then
    replace_placeholder "simse-api/wrangler.toml" "PLACEHOLDER_REPLACE_WITH_STORE_ID" "$prod_store_id"
    replace_placeholder "simse-mailer/wrangler.toml" "PLACEHOLDER_REPLACE_WITH_STORE_ID" "$prod_store_id"
fi

staging_store_id=$(get_value SECRETS_STORE_ID_STAGING "Enter staging Secrets Store ID (or Enter to use production):")
staging_store_id="${staging_store_id:-$prod_store_id}"
if [[ -n "$staging_store_id" ]]; then
    replace_placeholder "simse-api/wrangler.toml" "PLACEHOLDER_STAGING_STORE_ID" "$staging_store_id"
    replace_placeholder "simse-mailer/wrangler.toml" "PLACEHOLDER_STAGING_STORE_ID" "$staging_store_id"
fi

# ============================================================================
header "Step 5: Wrangler Secrets (simse-payments)"
# ============================================================================

echo "Setting wrangler secrets for simse-payments."
echo

set_worker_secret() {
    local worker="$1" secret_name="$2" env="$3" value="$4"
    if [[ -n "$value" ]]; then
        echo "$value" | $WRANGLER secret put "$secret_name" --name "$worker" --env "$env" 2>/dev/null
        ok "Set $secret_name on $worker ($env)"
    else
        warn "Skipped $secret_name ($env) — empty value"
    fi
}

if confirm "Set simse-payments secrets now?"; then

    # Production
    step "Setting production secrets..."
    prod_stripe_key=$(get_secret STRIPE_SECRET_KEY "Enter STRIPE_SECRET_KEY (production):")
    prod_stripe_webhook=$(get_secret STRIPE_WEBHOOK_SECRET "Enter STRIPE_WEBHOOK_SECRET (production):")
    prod_api_secret=$(get_secret API_SECRET "Enter API_SECRET (production):")
    prod_mailer_url=$(get_value MAILER_API_URL "Enter MAILER_API_URL (production):")
    prod_mailer_secret=$(get_secret MAILER_API_SECRET "Enter MAILER_API_SECRET (production):")

    set_worker_secret "simse-payments" "STRIPE_SECRET_KEY" "production" "$prod_stripe_key"
    set_worker_secret "simse-payments" "STRIPE_WEBHOOK_SECRET" "production" "$prod_stripe_webhook"
    set_worker_secret "simse-payments" "API_SECRET" "production" "$prod_api_secret"
    set_worker_secret "simse-payments" "MAILER_API_URL" "production" "$prod_mailer_url"
    set_worker_secret "simse-payments" "MAILER_API_SECRET" "production" "$prod_mailer_secret"

    # Staging — use staging-specific vars if set, otherwise fall back to production
    echo
    step "Setting staging secrets..."
    stg_stripe_key="${STRIPE_SECRET_KEY_STAGING:-$prod_stripe_key}"
    stg_stripe_webhook="${STRIPE_WEBHOOK_SECRET_STAGING:-$prod_stripe_webhook}"
    stg_api_secret="${API_SECRET_STAGING:-$prod_api_secret}"
    stg_mailer_url="${MAILER_API_URL_STAGING:-$prod_mailer_url}"
    stg_mailer_secret="${MAILER_API_SECRET_STAGING:-$prod_mailer_secret}"

    if [[ -n "${STRIPE_SECRET_KEY_STAGING:-}" ]]; then
        ok "Using staging-specific secrets from .env"
    else
        ok "Using production secrets as staging fallback"
    fi

    set_worker_secret "simse-payments" "STRIPE_SECRET_KEY" "staging" "$stg_stripe_key"
    set_worker_secret "simse-payments" "STRIPE_WEBHOOK_SECRET" "staging" "$stg_stripe_webhook"
    set_worker_secret "simse-payments" "API_SECRET" "staging" "$stg_api_secret"
    set_worker_secret "simse-payments" "MAILER_API_URL" "staging" "$stg_mailer_url"
    set_worker_secret "simse-payments" "MAILER_API_SECRET" "staging" "$stg_mailer_secret"

else
    warn "Skipped simse-payments secrets"
fi

# ============================================================================
header "Step 6: Verify Remaining Placeholders"
# ============================================================================

step "Scanning for remaining placeholders..."
remaining=$(grep -r "PLACEHOLDER" --include="wrangler.toml" "$REPO_ROOT" 2>/dev/null || true)

if [[ -n "$remaining" ]]; then
    warn "The following placeholders still need to be filled:"
    echo "$remaining" | while IFS= read -r line; do
        echo "    $line"
    done
else
    ok "All placeholders have been replaced!"
fi

# ============================================================================
header "Summary"
# ============================================================================

echo "GitHub secrets:"
gh secret list --repo "$GH_REPO" 2>/dev/null | while IFS= read -r line; do
    echo "  $line"
done

echo
echo "Next steps:"
echo "  1. Fill any remaining PLACEHOLDER values in wrangler.toml files"
echo "  2. Run D1 migrations for staging/production databases"
echo "  3. Commit updated wrangler.toml files"
echo "  4. Push to staging branch to trigger first staging deploy"
echo "  5. Push to production branch to trigger first production deploy"
echo

ok "Setup complete!"

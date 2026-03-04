-- Remove Stripe-related columns from teams (now in simse-payments)
ALTER TABLE teams DROP COLUMN stripe_customer_id;
ALTER TABLE teams DROP COLUMN stripe_subscription_id;

-- Drop credit_ledger (now in simse-payments)
DROP TABLE IF EXISTS credit_ledger;

-- A government transaction tax (e.g. an e-levy-style levy) is a liability, not
-- revenue: it is collected from the payer and owed to the tax authority. It
-- must be a distinct account from fee_revenue/platform_revenue so collected
-- tax is never confused with earned fees and can be remitted as a single
-- payment per period. One per currency, like the other platform singletons.
--
-- Design constraint from the master prompt (§1, §12.1): a per-country
-- transaction tax must be addable WITHOUT a schema change. This migration is
-- the only schema-touching piece; enabling/disabling the tax is configuration
-- (TaxPolicy.bps on the payment request), never a migration.
--
-- Account IDs follow the 0002 pattern
-- (11111111-1111-4111-8111-11111111xx0Y, x = account family, Y = currency).

INSERT INTO accounts (id, owner_type, owner_id, type, currency, name, status) VALUES
  ('11111111-1111-4111-8111-111111111501', 'platform', NULL, 'tax_payable', 'SLE', 'Transaction tax payable SLE', 'active'),
  ('11111111-1111-4111-8111-111111111502', 'platform', NULL, 'tax_payable', 'USD', 'Transaction tax payable USD', 'active');

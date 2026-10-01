-- Platform-controlled accounts, one set per currency. These are structural
-- singletons: owner_id NULL, unique per (type, currency), resolved by the
-- engine via (type, currency, owner_id IS NULL). Seeded once here so every
-- journal can always find its escrow / fee / bridge accounts.

INSERT INTO accounts (id, owner_type, owner_id, type, currency, name, status) VALUES
  ('11111111-1111-4111-8111-111111111101', 'platform', NULL, 'hold_escrow',      'SLE', 'Hold escrow SLE', 'active'),
  ('11111111-1111-4111-8111-111111111102', 'platform', NULL, 'hold_escrow',      'USD', 'Hold escrow USD', 'active'),
  ('11111111-1111-4111-8111-111111111201', 'platform', NULL, 'fee_revenue',      'SLE', 'Platform fee revenue SLE', 'active'),
  ('11111111-1111-4111-8111-111111111202', 'platform', NULL, 'fee_revenue',      'USD', 'Platform fee revenue USD', 'active'),
  ('11111111-1111-4111-8111-111111111301', 'platform', NULL, 'platform_revenue', 'SLE', 'Platform revenue SLE', 'active'),
  ('11111111-1111-4111-8111-111111111302', 'platform', NULL, 'platform_revenue', 'USD', 'Platform revenue USD', 'active'),
  ('11111111-1111-4111-8111-111111111401', 'platform', NULL, 'rail_bridge',      'SLE', 'Orange Money bridge SLE', 'active'),
  ('11111111-1111-4111-8111-111111111402', 'platform', NULL, 'rail_bridge',      'USD', 'Rail bridge USD', 'active');
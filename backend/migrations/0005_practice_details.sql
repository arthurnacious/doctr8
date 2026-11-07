-- =============================================================================
-- doctr8 — Practice contact and banking details
--
-- Printed on statements so patients know who to contact and where to pay.
-- All optional: a practice fills them in under settings. Bank details are
-- not secret (they go on every statement) but only practice managers can
-- change them.
-- =============================================================================

ALTER TABLE tenants
  ADD COLUMN address_line1       text CHECK (length(address_line1) <= 200),
  ADD COLUMN address_line2       text CHECK (length(address_line2) <= 200),
  ADD COLUMN suburb              text CHECK (length(suburb) <= 100),
  ADD COLUMN city                text CHECK (length(city) <= 100),
  ADD COLUMN postal_code         text CHECK (postal_code ~ '^[0-9]{4}$'),
  ADD COLUMN phone               text CHECK (phone ~ '^\+?[0-9]{7,15}$'),
  ADD COLUMN email               text CHECK (length(email) <= 254),
  ADD COLUMN bank_name           text CHECK (length(bank_name) <= 100),
  ADD COLUMN bank_account_name   text CHECK (length(bank_account_name) <= 100),
  ADD COLUMN bank_account_number text CHECK (bank_account_number ~ '^[0-9]{6,16}$'),
  ADD COLUMN bank_branch_code    text CHECK (bank_branch_code ~ '^[0-9]{6}$');

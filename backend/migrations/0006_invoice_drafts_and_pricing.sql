-- =============================================================================
-- doctr8 — Invoice drafts, tariff prices, ICD-10 age and sex rules
--
-- * Drafts: an invoice being prepared. Free-form (it may be incomplete),
--   editable and deletable, with no number and no money. Finalising
--   validates it, posts a real (immutable) invoice and deletes the draft.
-- * Tariff prices: global reference rates per tariff code, medical aid plan
--   and year. plan_id NULL is the default (private) rate. doctr8 maintains
--   them like the code lists.
-- * ICD-10 codes can be limited to one sex or an age range; the invoice
--   screen warns when they don't fit the patient.
-- =============================================================================

ALTER TABLE icd10_codes
  ADD COLUMN sex           text     CHECK (sex IN ('female', 'male')),
  ADD COLUMN min_age_years smallint CHECK (min_age_years BETWEEN 0 AND 130),
  ADD COLUMN max_age_years smallint CHECK (max_age_years BETWEEN 0 AND 130),
  ADD CONSTRAINT icd10_codes_age_range_check
    CHECK (min_age_years IS NULL OR max_age_years IS NULL OR min_age_years <= max_age_years);

CREATE TABLE tariff_prices (
  id             uuid     PRIMARY KEY DEFAULT gen_random_uuid(),
  tariff_code_id uuid     NOT NULL REFERENCES tariff_codes (id) ON DELETE RESTRICT,
  -- NULL: the default rate (private patients, or plans without their own).
  plan_id        uuid     REFERENCES medical_scheme_plans (id) ON DELETE RESTRICT,
  year           smallint NOT NULL CHECK (year BETWEEN 2000 AND 2100),
  price_cents    bigint   NOT NULL CHECK (price_cents BETWEEN 0 AND 100000000000),
  CONSTRAINT tariff_prices_uq UNIQUE NULLS NOT DISTINCT (tariff_code_id, plan_id, year)
);

GRANT SELECT ON tariff_prices TO doctr8_app;

CREATE TABLE invoice_drafts (
  id         uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id  uuid        NOT NULL DEFAULT app.current_tenant_id()
                         REFERENCES tenants (id) ON DELETE RESTRICT,
  account_id uuid        NOT NULL,
  -- The invoice as entered so far, plus screen-only details (descriptions
  -- shown next to codes). Validated only when finalised.
  content    jsonb       NOT NULL CHECK (jsonb_typeof(content) = 'object'
                                         AND octet_length(content::text) <= 200000),
  created_by uuid        REFERENCES users (id) ON DELETE SET NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now(),
  FOREIGN KEY (tenant_id, account_id) REFERENCES accounts (tenant_id, id)
);

CREATE INDEX invoice_drafts_account_idx ON invoice_drafts (tenant_id, account_id);

CREATE TRIGGER invoice_drafts_touch BEFORE UPDATE ON invoice_drafts
  FOR EACH ROW EXECUTE FUNCTION app.touch_updated_at();

ALTER TABLE invoice_drafts ENABLE ROW LEVEL SECURITY;
ALTER TABLE invoice_drafts FORCE  ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON invoice_drafts
  USING (tenant_id = app.current_tenant_id())
  WITH CHECK (tenant_id = app.current_tenant_id());

-- Drafts aren't posted records: they can be changed and discarded.
GRANT SELECT, INSERT, DELETE ON invoice_drafts TO doctr8_app;
GRANT UPDATE (content) ON invoice_drafts TO doctr8_app;

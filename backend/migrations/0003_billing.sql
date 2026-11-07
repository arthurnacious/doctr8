-- =============================================================================
-- doctr8 — Billing & Statements
--
-- Ledger model
--   transactions         one row per business event (invoice charge, claim
--                        submitted, remittance, payment, refund, credit note,
--                        journal, liability transfer, reversal)
--   transaction_entries  signed amounts per invoice and *bucket*
--                        (medical_aid | patient). Positive = more owed.
--                        An entry with no invoice is unallocated (patient
--                        credit or debit on the account).
--
--   What the medical aid / patient owes on an invoice = SUM(entries) for
--   that invoice and bucket. Nothing is stored twice.
--
-- Posted money is immutable: transactions, entries, invoices, lines and
-- credit notes reject UPDATE/DELETE/TRUNCATE (trigger, on top of grants).
-- Corrections are new transactions (reversal, journal, credit note).
--
-- Rule violations raise SQLSTATE D8xxx with messages free of personal data.
-- =============================================================================

DO $$
BEGIN
  IF NOT (SELECT rolsuper OR rolbypassrls FROM pg_roles WHERE rolname = current_user) THEN
    RAISE EXCEPTION 'Run migrations as a superuser or a BYPASSRLS role';
  END IF;
END
$$;

-- -----------------------------------------------------------------------------
-- Permissions (+ grant to every existing Owner role)
-- -----------------------------------------------------------------------------

INSERT INTO permissions (key, description) VALUES
  ('billing.view',     'View invoices, claims, transactions and statements'),
  ('billing.manage',   'Create invoices and credit notes; record claims and claim status'),
  ('payments.manage',  'Record patient payments, refunds and medical aid remittances'),
  ('journals.manage',  'Post journals, liability transfers and reversals; edit journal reasons'),
  ('providers.manage', 'Manage providers and service centres');

INSERT INTO role_permissions (tenant_id, role_id, permission_key)
SELECT r.tenant_id, r.id, p.key
  FROM roles r
 CROSS JOIN permissions p
 WHERE r.is_system
   AND p.key IN ('billing.view', 'billing.manage', 'payments.manage', 'journals.manage', 'providers.manage')
ON CONFLICT DO NOTHING;

-- Lets invoices reference (tenant, account, patient) so the patient must be
-- on the invoiced account.
ALTER TABLE patients ADD CONSTRAINT patients_account_patient_uq UNIQUE (tenant_id, account_id, id);

-- Advisory-lock key for an entity. Money postings on an account (and
-- allocations of a remittance) serialise on these, from the API and from
-- the triggers below.
CREATE FUNCTION app.lock_key(p_id uuid) RETURNS bigint
LANGUAGE sql IMMUTABLE PARALLEL SAFE AS $$
  SELECT hashtextextended(p_id::text, 8)
$$;

-- =============================================================================
-- Coding reference (global; doctr8 maintains it, practices read it)
-- Minimal for now: full list imports come later.
-- =============================================================================

CREATE TABLE icd10_codes (
  code             text    PRIMARY KEY CHECK (code ~ '^[A-Z][0-9]{2}(\.[0-9A-Z]{1,4})?$'),
  description      text    NOT NULL,
  valid_as_primary boolean NOT NULL DEFAULT true,
  is_active        boolean NOT NULL DEFAULT true
);

CREATE TABLE tariff_codes (
  id          uuid    PRIMARY KEY DEFAULT gen_random_uuid(),
  -- Which coding structure the code belongs to (e.g. CCSA). Codes repeat
  -- across code sets with different meanings.
  code_set    text    NOT NULL DEFAULT 'CCSA' CHECK (code_set ~ '^[A-Z0-9_]{2,20}$'),
  code        text    NOT NULL CHECK (code ~ '^[0-9A-Z]{4,6}$'),
  description text    NOT NULL,
  is_active   boolean NOT NULL DEFAULT true,
  CONSTRAINT tariff_codes_code_uq UNIQUE (code_set, code)
);

CREATE TABLE nappi_codes (
  -- 6-digit product code, optionally + 3-digit pack suffix.
  code        text    PRIMARY KEY CHECK (code ~ '^[0-9]{6}([0-9]{3})?$'),
  description text    NOT NULL,
  is_active   boolean NOT NULL DEFAULT true
);

CREATE TABLE modifiers (
  code        text    PRIMARY KEY CHECK (code ~ '^[0-9]{4}$'),
  description text    NOT NULL,
  is_active   boolean NOT NULL DEFAULT true
);

-- =============================================================================
-- Providers and service centres
-- =============================================================================

CREATE TABLE providers (
  id              uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id       uuid        NOT NULL DEFAULT app.current_tenant_id()
                              REFERENCES tenants (id) ON DELETE RESTRICT,
  -- practice: the practice's own treating doctors; external: referring
  -- doctors, surgical assistants, anyone else named on an invoice.
  kind            text        NOT NULL CHECK (kind IN ('practice', 'external')),
  title           text        CHECK (length(title) <= 20),
  first_name      text,
  last_name       text        NOT NULL CHECK (length(btrim(last_name)) > 0),
  -- BHF PCNS practice number.
  practice_number text        CHECK (practice_number ~ '^[0-9]{7}$'),
  -- HPCSA council registration number, e.g. MP0123456.
  hpcsa_number    text        CHECK (hpcsa_number ~ '^[A-Z]{2,4}[0-9]{4,8}$'),
  speciality      text,
  -- The practice member this provider logs in as, if any.
  user_id         uuid,
  is_active       boolean     NOT NULL DEFAULT true,
  created_by      uuid        REFERENCES users (id) ON DELETE SET NULL,
  created_at      timestamptz NOT NULL DEFAULT now(),
  updated_at      timestamptz NOT NULL DEFAULT now(),
  CONSTRAINT providers_user_kind_check CHECK (kind = 'practice' OR user_id IS NULL),
  UNIQUE (tenant_id, id),
  -- A linked user must be a member of this practice.
  FOREIGN KEY (tenant_id, user_id) REFERENCES memberships (tenant_id, user_id)
);

CREATE UNIQUE INDEX providers_user_uq  ON providers (tenant_id, user_id) WHERE user_id IS NOT NULL;
CREATE UNIQUE INDEX providers_hpcsa_uq ON providers (tenant_id, hpcsa_number) WHERE hpcsa_number IS NOT NULL;
CREATE INDEX providers_name_idx ON providers (tenant_id, lower(last_name));

CREATE TRIGGER providers_touch BEFORE UPDATE ON providers
  FOR EACH ROW EXECUTE FUNCTION app.touch_updated_at();

CREATE TABLE service_centres (
  id                       uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id                uuid        NOT NULL DEFAULT app.current_tenant_id()
                                       REFERENCES tenants (id) ON DELETE RESTRICT,
  kind                     text        NOT NULL CHECK (kind IN ('rooms', 'hospital', 'day_clinic', 'other')),
  name                     text        NOT NULL CHECK (length(btrim(name)) > 0),
  -- The facility's own BHF number (hospitals).
  practice_number          text        CHECK (practice_number ~ '^[0-9]{7}$'),
  -- Default place-of-service code for invoice lines (11 rooms, 21 hospital ...).
  default_place_of_service text        CHECK (default_place_of_service ~ '^[0-9]{2}$'),
  is_active                boolean     NOT NULL DEFAULT true,
  created_at               timestamptz NOT NULL DEFAULT now(),
  updated_at               timestamptz NOT NULL DEFAULT now(),
  UNIQUE (tenant_id, id)
);

CREATE UNIQUE INDEX service_centres_name_uq ON service_centres (tenant_id, lower(name));

CREATE TRIGGER service_centres_touch BEFORE UPDATE ON service_centres
  FOR EACH ROW EXECUTE FUNCTION app.touch_updated_at();

-- =============================================================================
-- Journal reason types (tenant-editable list)
-- =============================================================================

CREATE TABLE journal_reason_types (
  id         uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id  uuid        NOT NULL DEFAULT app.current_tenant_id()
                         REFERENCES tenants (id) ON DELETE RESTRICT,
  code       text        NOT NULL CHECK (code ~ '^[a-z][a-z0-9_]{1,39}$'),
  name       text        NOT NULL CHECK (length(btrim(name)) > 0),
  is_active  boolean     NOT NULL DEFAULT true,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE (tenant_id, id),
  CONSTRAINT journal_reason_types_code_uq UNIQUE (tenant_id, code)
);

CREATE TRIGGER journal_reason_types_touch BEFORE UPDATE ON journal_reason_types
  FOR EACH ROW EXECUTE FUNCTION app.touch_updated_at();

-- Per-practice document numbers.
CREATE TABLE billing_counters (
  tenant_id  uuid   NOT NULL DEFAULT app.current_tenant_id()
                    REFERENCES tenants (id) ON DELETE RESTRICT,
  kind       text   NOT NULL CHECK (kind IN ('invoice', 'credit_note')),
  last_value bigint NOT NULL,
  PRIMARY KEY (tenant_id, kind)
);

-- =============================================================================
-- Invoices
-- =============================================================================

CREATE TABLE invoices (
  id                    uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id             uuid        NOT NULL DEFAULT app.current_tenant_id()
                                    REFERENCES tenants (id) ON DELETE RESTRICT,
  account_id            uuid        NOT NULL,
  patient_id            uuid        NOT NULL,
  invoice_no            bigint      NOT NULL CHECK (invoice_no > 0),
  invoice_date          date        NOT NULL,
  treating_provider_id  uuid        NOT NULL,
  referring_provider_id uuid,
  service_centre_id     uuid        NOT NULL,
  authorisation_number  text        CHECK (authorisation_number ~ '^[A-Za-z0-9/-]{1,40}$'),
  total_cents           bigint      NOT NULL CHECK (total_cents > 0),
  created_by            uuid        REFERENCES users (id) ON DELETE SET NULL,
  created_at            timestamptz NOT NULL DEFAULT now(),
  UNIQUE (tenant_id, id),
  UNIQUE (tenant_id, account_id, id),
  CONSTRAINT invoices_number_uq UNIQUE (tenant_id, invoice_no),
  FOREIGN KEY (tenant_id, account_id, patient_id) REFERENCES patients (tenant_id, account_id, id),
  FOREIGN KEY (tenant_id, treating_provider_id)  REFERENCES providers (tenant_id, id),
  FOREIGN KEY (tenant_id, referring_provider_id) REFERENCES providers (tenant_id, id),
  FOREIGN KEY (tenant_id, service_centre_id)     REFERENCES service_centres (tenant_id, id)
);

CREATE INDEX invoices_account_idx ON invoices (tenant_id, account_id, invoice_date);

CREATE TABLE invoice_lines (
  id                    uuid          PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id             uuid          NOT NULL DEFAULT app.current_tenant_id()
                                      REFERENCES tenants (id) ON DELETE RESTRICT,
  invoice_id            uuid          NOT NULL,
  line_no               smallint      NOT NULL CHECK (line_no > 0),
  service_date          date          NOT NULL,
  tariff_code_id        uuid          REFERENCES tariff_codes (id),
  nappi_code            text          REFERENCES nappi_codes (code),
  description           text          NOT NULL CHECK (length(btrim(description)) > 0),
  quantity              numeric(10,3) NOT NULL CHECK (quantity > 0),
  unit_price_cents      bigint        NOT NULL CHECK (unit_price_cents >= 0),
  amount_cents          bigint        NOT NULL CHECK (amount_cents >= 0),
  place_of_service      text          NOT NULL CHECK (place_of_service ~ '^[0-9]{2}$'),
  assistant_provider_id uuid,
  CONSTRAINT invoice_lines_item_check CHECK ((tariff_code_id IS NULL) <> (nappi_code IS NULL)),
  UNIQUE (tenant_id, id),
  UNIQUE (tenant_id, invoice_id, id),
  CONSTRAINT invoice_lines_no_uq UNIQUE (tenant_id, invoice_id, line_no),
  FOREIGN KEY (tenant_id, invoice_id) REFERENCES invoices (tenant_id, id),
  FOREIGN KEY (tenant_id, assistant_provider_id) REFERENCES providers (tenant_id, id)
);

CREATE TABLE invoice_line_icd10 (
  tenant_id  uuid     NOT NULL DEFAULT app.current_tenant_id(),
  line_id    uuid     NOT NULL,
  -- 1 = primary diagnosis.
  position   smallint NOT NULL CHECK (position BETWEEN 1 AND 12),
  icd10_code text     NOT NULL REFERENCES icd10_codes (code),
  PRIMARY KEY (tenant_id, line_id, position),
  CONSTRAINT invoice_line_icd10_code_uq UNIQUE (tenant_id, line_id, icd10_code),
  FOREIGN KEY (tenant_id, line_id) REFERENCES invoice_lines (tenant_id, id)
);

CREATE TABLE invoice_line_modifiers (
  tenant_id     uuid     NOT NULL DEFAULT app.current_tenant_id(),
  line_id       uuid     NOT NULL,
  position      smallint NOT NULL CHECK (position BETWEEN 1 AND 10),
  modifier_code text     NOT NULL REFERENCES modifiers (code),
  PRIMARY KEY (tenant_id, line_id, position),
  CONSTRAINT invoice_line_modifiers_code_uq UNIQUE (tenant_id, line_id, modifier_code),
  FOREIGN KEY (tenant_id, line_id) REFERENCES invoice_lines (tenant_id, id)
);

-- Credit notes reverse individual lines (fully or partly).
CREATE TABLE credit_notes (
  id             uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id      uuid        NOT NULL DEFAULT app.current_tenant_id()
                             REFERENCES tenants (id) ON DELETE RESTRICT,
  invoice_id     uuid        NOT NULL,
  credit_note_no bigint      NOT NULL CHECK (credit_note_no > 0),
  credit_date    date        NOT NULL,
  reason         text        NOT NULL CHECK (length(btrim(reason)) > 0),
  total_cents    bigint      NOT NULL CHECK (total_cents > 0),
  created_by     uuid        REFERENCES users (id) ON DELETE SET NULL,
  created_at     timestamptz NOT NULL DEFAULT now(),
  UNIQUE (tenant_id, id),
  UNIQUE (tenant_id, id, invoice_id),
  CONSTRAINT credit_notes_number_uq UNIQUE (tenant_id, credit_note_no),
  FOREIGN KEY (tenant_id, invoice_id) REFERENCES invoices (tenant_id, id)
);

CREATE TABLE credit_note_lines (
  id              uuid   PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id       uuid   NOT NULL DEFAULT app.current_tenant_id(),
  credit_note_id  uuid   NOT NULL,
  invoice_id      uuid   NOT NULL,
  invoice_line_id uuid   NOT NULL,
  amount_cents    bigint NOT NULL CHECK (amount_cents > 0),
  CONSTRAINT credit_note_lines_line_uq UNIQUE (tenant_id, credit_note_id, invoice_line_id),
  -- Both FKs carry invoice_id: the credited line must belong to the
  -- credit note's invoice.
  FOREIGN KEY (tenant_id, credit_note_id, invoice_id) REFERENCES credit_notes (tenant_id, id, invoice_id),
  FOREIGN KEY (tenant_id, invoice_id, invoice_line_id) REFERENCES invoice_lines (tenant_id, invoice_id, id)
);

CREATE INDEX credit_note_lines_line_idx ON credit_note_lines (tenant_id, invoice_line_id);

-- =============================================================================
-- Claims (submission to switching houses comes later; we only record it)
-- =============================================================================

CREATE TABLE claims (
  id             uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id      uuid        NOT NULL DEFAULT app.current_tenant_id()
                             REFERENCES tenants (id) ON DELETE RESTRICT,
  invoice_id     uuid        NOT NULL,
  status         text        NOT NULL DEFAULT 'submitted'
                             CHECK (status IN ('submitted', 'accepted', 'rejected', 'paid')),
  claimed_cents  bigint      NOT NULL CHECK (claimed_cents > 0),
  submitted_date date        NOT NULL,
  status_reason  text,
  created_by     uuid        REFERENCES users (id) ON DELETE SET NULL,
  created_at     timestamptz NOT NULL DEFAULT now(),
  updated_at     timestamptz NOT NULL DEFAULT now(),
  UNIQUE (tenant_id, id),
  FOREIGN KEY (tenant_id, invoice_id) REFERENCES invoices (tenant_id, id)
);

-- One open claim per invoice; resubmitting after rejection is a new claim.
CREATE UNIQUE INDEX claims_open_uq ON claims (tenant_id, invoice_id) WHERE status IN ('submitted', 'accepted');
CREATE INDEX claims_status_idx ON claims (tenant_id, status, updated_at);

CREATE TRIGGER claims_touch BEFORE UPDATE ON claims
  FOR EACH ROW EXECUTE FUNCTION app.touch_updated_at();

CREATE TABLE claim_status_history (
  id         uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id  uuid        NOT NULL DEFAULT app.current_tenant_id(),
  claim_id   uuid        NOT NULL,
  status     text        NOT NULL,
  reason     text,
  changed_by uuid        REFERENCES users (id) ON DELETE SET NULL,
  changed_at timestamptz NOT NULL DEFAULT now(),
  FOREIGN KEY (tenant_id, claim_id) REFERENCES claims (tenant_id, id)
);

CREATE INDEX claim_status_history_claim_idx ON claim_status_history (tenant_id, claim_id, changed_at);

-- Allowed transitions; history is written by the database so it can't be skipped.
CREATE FUNCTION app.claims_status() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
  IF TG_OP = 'INSERT' THEN
    IF NEW.status <> 'submitted' THEN
      RAISE EXCEPTION USING ERRCODE = 'D8301', MESSAGE = 'A new claim starts as submitted';
    END IF;
  ELSIF NEW.status IS DISTINCT FROM OLD.status THEN
    IF NOT ((OLD.status = 'submitted' AND NEW.status IN ('accepted', 'rejected', 'paid'))
         OR (OLD.status = 'accepted'  AND NEW.status IN ('paid', 'rejected'))) THEN
      RAISE EXCEPTION USING ERRCODE = 'D8301',
        MESSAGE = format('A claim can''t go from %s to %s', OLD.status, NEW.status);
    END IF;
  ELSE
    RETURN NEW;
  END IF;
  INSERT INTO claim_status_history (tenant_id, claim_id, status, reason, changed_by)
  VALUES (NEW.tenant_id, NEW.id, NEW.status, NEW.status_reason, app.current_user_id());
  RETURN NEW;
END
$$;

CREATE TRIGGER claims_status AFTER INSERT OR UPDATE ON claims
  FOR EACH ROW EXECUTE FUNCTION app.claims_status();

-- =============================================================================
-- Remittances (medical aid payments)
-- =============================================================================

CREATE TABLE remittances (
  id            uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id     uuid        NOT NULL DEFAULT app.current_tenant_id()
                            REFERENCES tenants (id) ON DELETE RESTRICT,
  scheme_id     uuid        NOT NULL REFERENCES medical_schemes (id),
  -- Remittance advice / EFT reference.
  reference     text        NOT NULL CHECK (length(btrim(reference)) BETWEEN 1 AND 60),
  received_date date        NOT NULL,
  total_cents   bigint      NOT NULL CHECK (total_cents > 0),
  notes         text,
  created_by    uuid        REFERENCES users (id) ON DELETE SET NULL,
  created_at    timestamptz NOT NULL DEFAULT now(),
  UNIQUE (tenant_id, id),
  CONSTRAINT remittances_reference_uq UNIQUE (tenant_id, scheme_id, reference)
);

-- =============================================================================
-- Ledger
-- =============================================================================

CREATE TABLE transactions (
  id                      uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id               uuid        NOT NULL DEFAULT app.current_tenant_id()
                                      REFERENCES tenants (id) ON DELETE RESTRICT,
  account_id              uuid        NOT NULL,
  kind                    text        NOT NULL CHECK (kind IN (
                            'invoice', 'claim_submitted', 'remittance', 'patient_payment', 'refund',
                            'credit_note', 'journal', 'liability_transfer', 'reversal')),
  -- Business date (what statements and ageing use).
  txn_date                date        NOT NULL,
  posted_at               timestamptz NOT NULL DEFAULT now(),
  -- Headline amount, always >= 0 (payment received, amount claimed, ...).
  amount_cents            bigint      NOT NULL CHECK (amount_cents >= 0),
  payment_method          text        CHECK (payment_method IN ('cash', 'card', 'eft')),
  reference               text        CHECK (length(reference) <= 60),
  notes                   text,
  invoice_id              uuid,
  claim_id                uuid,
  credit_note_id          uuid,
  remittance_id           uuid,
  reason_type_id          uuid,
  reverses_transaction_id uuid        UNIQUE,
  created_by              uuid        REFERENCES users (id) ON DELETE SET NULL,
  UNIQUE (tenant_id, id),
  UNIQUE (tenant_id, id, account_id),
  CONSTRAINT transactions_method_check     CHECK ((kind IN ('patient_payment', 'refund')) = (payment_method IS NOT NULL)),
  CONSTRAINT transactions_reason_check     CHECK ((kind = 'journal') = (reason_type_id IS NOT NULL)),
  CONSTRAINT transactions_reversal_check   CHECK ((kind = 'reversal') = (reverses_transaction_id IS NOT NULL)),
  CONSTRAINT transactions_credit_check     CHECK ((kind = 'credit_note') = (credit_note_id IS NOT NULL)),
  CONSTRAINT transactions_claim_check      CHECK ((kind = 'claim_submitted') = (claim_id IS NOT NULL)),
  CONSTRAINT transactions_remittance_check CHECK ((kind = 'remittance' AND remittance_id IS NOT NULL)
                                               OR (kind = 'reversal')
                                               OR remittance_id IS NULL),
  CONSTRAINT transactions_invoice_check    CHECK (kind NOT IN ('invoice', 'claim_submitted', 'credit_note', 'liability_transfer')
                                               OR invoice_id IS NOT NULL),
  CONSTRAINT transactions_journal_notes_check CHECK (kind NOT IN ('journal', 'reversal', 'liability_transfer')
                                               OR length(btrim(notes)) > 0),
  FOREIGN KEY (tenant_id, account_id) REFERENCES accounts (tenant_id, id),
  FOREIGN KEY (tenant_id, account_id, invoice_id) REFERENCES invoices (tenant_id, account_id, id),
  FOREIGN KEY (tenant_id, claim_id) REFERENCES claims (tenant_id, id),
  FOREIGN KEY (tenant_id, credit_note_id) REFERENCES credit_notes (tenant_id, id),
  FOREIGN KEY (tenant_id, remittance_id) REFERENCES remittances (tenant_id, id),
  FOREIGN KEY (tenant_id, reason_type_id) REFERENCES journal_reason_types (tenant_id, id),
  -- A reversal is on the same account as what it reverses.
  FOREIGN KEY (tenant_id, reverses_transaction_id, account_id) REFERENCES transactions (tenant_id, id, account_id)
);

CREATE INDEX transactions_account_idx    ON transactions (tenant_id, account_id, txn_date);
CREATE INDEX transactions_remittance_idx ON transactions (tenant_id, remittance_id) WHERE remittance_id IS NOT NULL;

CREATE TABLE transaction_entries (
  id             uuid   PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id      uuid   NOT NULL DEFAULT app.current_tenant_id(),
  transaction_id uuid   NOT NULL,
  account_id     uuid   NOT NULL,
  invoice_id     uuid,
  bucket         text   NOT NULL CHECK (bucket IN ('medical_aid', 'patient')),
  amount_cents   bigint NOT NULL CHECK (amount_cents <> 0),
  -- Only the patient side can hold unallocated money.
  CONSTRAINT transaction_entries_unallocated_check CHECK (invoice_id IS NOT NULL OR bucket = 'patient'),
  -- Same account as the transaction, and the invoice is on that account.
  FOREIGN KEY (tenant_id, transaction_id, account_id) REFERENCES transactions (tenant_id, id, account_id),
  FOREIGN KEY (tenant_id, account_id, invoice_id) REFERENCES invoices (tenant_id, account_id, id)
);

CREATE INDEX transaction_entries_txn_idx     ON transaction_entries (tenant_id, transaction_id);
CREATE INDEX transaction_entries_account_idx ON transaction_entries (tenant_id, account_id);
CREATE INDEX transaction_entries_invoice_idx ON transaction_entries (tenant_id, invoice_id) WHERE invoice_id IS NOT NULL;

-- =============================================================================
-- Immutability
-- =============================================================================

CREATE FUNCTION app.forbid_change() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
  RAISE EXCEPTION USING ERRCODE = 'D8101',
    MESSAGE = format('%s are immutable once posted; post a correcting transaction instead', TG_TABLE_NAME);
END
$$;

DO $$
DECLARE
  t text;
BEGIN
  FOREACH t IN ARRAY ARRAY['transactions', 'transaction_entries', 'invoices', 'invoice_lines',
                           'invoice_line_icd10', 'invoice_line_modifiers', 'credit_notes',
                           'credit_note_lines', 'claim_status_history', 'remittances']
  LOOP
    EXECUTE format('CREATE TRIGGER %I BEFORE UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION app.forbid_change()',
                   t || '_immutable', t);
    EXECUTE format('CREATE TRIGGER %I BEFORE TRUNCATE ON %I FOR EACH STATEMENT EXECUTE FUNCTION app.forbid_change()',
                   t || '_no_truncate', t);
  END LOOP;
END
$$;

-- =============================================================================
-- Ledger rules, re-checked at COMMIT (deferred constraint triggers)
-- =============================================================================

-- Serialise postings per account (and per remittance) even for SQL that
-- bypasses the API.
CREATE FUNCTION app.transactions_lock() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
  PERFORM pg_advisory_xact_lock(app.lock_key(NEW.account_id));
  IF NEW.remittance_id IS NOT NULL THEN
    PERFORM pg_advisory_xact_lock(app.lock_key(NEW.remittance_id));
  END IF;
  RETURN NEW;
END
$$;

CREATE TRIGGER transactions_lock BEFORE INSERT ON transactions
  FOR EACH ROW EXECUTE FUNCTION app.transactions_lock();

CREATE FUNCTION app.validate_transaction(p_tenant uuid, p_txn uuid) RETURNS void
LANGUAGE plpgsql AS $$
DECLARE
  t       transactions%ROWTYPE;
  o       transactions%ROWTYPE;
  v_n     int;
  v_sum   bigint;
  v_abs   bigint;
  v_pos   int;
  v_neg   int;
  v_ma    int;
  v_pt    int;
  v_other int;
  v_total bigint;
  v_alloc bigint;
BEGIN
  SELECT * INTO t FROM transactions WHERE tenant_id = p_tenant AND id = p_txn;
  IF NOT FOUND THEN
    RETURN;
  END IF;

  SELECT count(*),
         coalesce(sum(amount_cents), 0)::bigint,
         coalesce(sum(abs(amount_cents)), 0)::bigint,
         count(*) FILTER (WHERE amount_cents > 0),
         count(*) FILTER (WHERE amount_cents < 0),
         count(*) FILTER (WHERE bucket = 'medical_aid'),
         count(*) FILTER (WHERE bucket = 'patient'),
         count(*) FILTER (WHERE invoice_id IS DISTINCT FROM t.invoice_id)
    INTO v_n, v_sum, v_abs, v_pos, v_neg, v_ma, v_pt, v_other
    FROM transaction_entries
   WHERE tenant_id = p_tenant AND transaction_id = p_txn;

  IF t.kind = 'claim_submitted' THEN
    IF v_n > 0 THEN
      RAISE EXCEPTION USING ERRCODE = 'D8401', MESSAGE = 'Recording a claim submission doesn''t move money';
    END IF;
    RETURN;
  END IF;

  IF v_n = 0 THEN
    RAISE EXCEPTION USING ERRCODE = 'D8402', MESSAGE = format('A %s transaction needs at least one ledger entry', t.kind);
  END IF;

  CASE t.kind
    WHEN 'invoice' THEN
      SELECT total_cents INTO v_total FROM invoices WHERE tenant_id = p_tenant AND id = t.invoice_id;
      IF v_n <> 1 OR v_neg > 0 OR v_sum <> v_total OR v_other > 0 OR t.amount_cents <> v_total THEN
        RAISE EXCEPTION USING ERRCODE = 'D8403', MESSAGE = 'An invoice charge posts its full total once, to its own invoice';
      END IF;
    WHEN 'credit_note' THEN
      SELECT total_cents INTO v_total FROM credit_notes WHERE tenant_id = p_tenant AND id = t.credit_note_id;
      IF v_pos > 0 OR v_sum <> -v_total OR v_other > 0 OR t.amount_cents <> v_total THEN
        RAISE EXCEPTION USING ERRCODE = 'D8404', MESSAGE = 'A credit note reduces its own invoice by exactly its total';
      END IF;
    WHEN 'remittance' THEN
      IF v_pt > 0 OR v_pos > 0 OR t.amount_cents <> -v_sum THEN
        RAISE EXCEPTION USING ERRCODE = 'D8405', MESSAGE = 'A remittance only reduces what the medical aid owes';
      END IF;
    WHEN 'patient_payment' THEN
      IF v_ma > 0 OR v_pos > 0 OR t.amount_cents <> -v_sum THEN
        RAISE EXCEPTION USING ERRCODE = 'D8406', MESSAGE = 'A patient payment only reduces what the patient owes';
      END IF;
    WHEN 'refund' THEN
      IF v_ma > 0 OR v_neg > 0 OR t.amount_cents <> v_sum THEN
        RAISE EXCEPTION USING ERRCODE = 'D8407', MESSAGE = 'A refund only increases what the patient owes';
      END IF;
    WHEN 'liability_transfer' THEN
      IF v_n <> 2 OR v_ma <> 1 OR v_pt <> 1 OR v_sum <> 0 OR v_other > 0 OR t.amount_cents * 2 <> v_abs THEN
        RAISE EXCEPTION USING ERRCODE = 'D8408',
          MESSAGE = 'A liability transfer moves one amount between medical aid and patient on one invoice';
      END IF;
    WHEN 'journal' THEN
      IF t.amount_cents <> v_abs THEN
        RAISE EXCEPTION USING ERRCODE = 'D8414', MESSAGE = 'A journal''s amount is the sum of its entries';
      END IF;
    WHEN 'reversal' THEN
      SELECT * INTO o FROM transactions WHERE tenant_id = p_tenant AND id = t.reverses_transaction_id;
      IF o.kind IN ('invoice', 'claim_submitted', 'credit_note', 'reversal') THEN
        RAISE EXCEPTION USING ERRCODE = 'D8409', MESSAGE = format('%s transactions can''t be reversed', o.kind);
      END IF;
      IF t.txn_date < o.txn_date THEN
        RAISE EXCEPTION USING ERRCODE = 'D8410', MESSAGE = 'A reversal can''t be dated before the transaction it reverses';
      END IF;
      IF t.remittance_id IS DISTINCT FROM o.remittance_id OR t.amount_cents <> o.amount_cents THEN
        RAISE EXCEPTION USING ERRCODE = 'D8411', MESSAGE = 'A reversal carries the original''s remittance and amount';
      END IF;
      IF EXISTS (SELECT invoice_id, bucket, -amount_cents FROM transaction_entries
                  WHERE tenant_id = p_tenant AND transaction_id = o.id
                 EXCEPT ALL
                 SELECT invoice_id, bucket, amount_cents FROM transaction_entries
                  WHERE tenant_id = p_tenant AND transaction_id = t.id)
      OR EXISTS (SELECT invoice_id, bucket, amount_cents FROM transaction_entries
                  WHERE tenant_id = p_tenant AND transaction_id = t.id
                 EXCEPT ALL
                 SELECT invoice_id, bucket, -amount_cents FROM transaction_entries
                  WHERE tenant_id = p_tenant AND transaction_id = o.id) THEN
        RAISE EXCEPTION USING ERRCODE = 'D8412', MESSAGE = 'A reversal must exactly negate the original''s entries';
      END IF;
  END CASE;

  IF t.remittance_id IS NOT NULL THEN
    SELECT total_cents INTO v_total FROM remittances WHERE tenant_id = p_tenant AND id = t.remittance_id;
    SELECT coalesce(-sum(e.amount_cents), 0)::bigint INTO v_alloc
      FROM transaction_entries e
      JOIN transactions x ON x.tenant_id = e.tenant_id AND x.id = e.transaction_id
     WHERE x.tenant_id = p_tenant AND x.remittance_id = t.remittance_id;
    IF v_alloc > v_total THEN
      RAISE EXCEPTION USING ERRCODE = 'D8413', MESSAGE = 'Allocations exceed the remittance total';
    END IF;
  END IF;
END
$$;

CREATE FUNCTION app.transactions_validate() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
  PERFORM app.validate_transaction(NEW.tenant_id, NEW.id);
  RETURN NULL;
END
$$;

CREATE FUNCTION app.transaction_entries_validate() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
  PERFORM app.validate_transaction(NEW.tenant_id, NEW.transaction_id);
  RETURN NULL;
END
$$;

CREATE CONSTRAINT TRIGGER transactions_rules AFTER INSERT ON transactions
  DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION app.transactions_validate();
CREATE CONSTRAINT TRIGGER transaction_entries_rules AFTER INSERT ON transaction_entries
  DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION app.transaction_entries_validate();

-- Invoice: has lines, total = sum of lines, every line has a diagnosis, and
-- it is charged to the ledger exactly once.
CREATE FUNCTION app.validate_invoice(p_tenant uuid, p_invoice uuid) RETURNS void
LANGUAGE plpgsql AS $$
DECLARE
  v_total   bigint;
  v_lines   int;
  v_sum     bigint;
  v_no_dx   int;
  v_charges int;
BEGIN
  SELECT total_cents INTO v_total FROM invoices WHERE tenant_id = p_tenant AND id = p_invoice;
  IF NOT FOUND THEN
    RETURN;
  END IF;
  SELECT count(*),
         coalesce(sum(l.amount_cents), 0)::bigint,
         count(*) FILTER (WHERE NOT EXISTS (
           SELECT 1 FROM invoice_line_icd10 d WHERE d.tenant_id = l.tenant_id AND d.line_id = l.id))
    INTO v_lines, v_sum, v_no_dx
    FROM invoice_lines l
   WHERE l.tenant_id = p_tenant AND l.invoice_id = p_invoice;

  IF v_lines = 0 THEN
    RAISE EXCEPTION USING ERRCODE = 'D8201', MESSAGE = 'An invoice needs at least one line';
  END IF;
  IF v_sum <> v_total THEN
    RAISE EXCEPTION USING ERRCODE = 'D8202', MESSAGE = 'An invoice total must equal the sum of its lines';
  END IF;
  IF v_no_dx > 0 THEN
    RAISE EXCEPTION USING ERRCODE = 'D8203', MESSAGE = 'Every invoice line needs at least one ICD-10 code';
  END IF;
  SELECT count(*) INTO v_charges
    FROM transactions WHERE tenant_id = p_tenant AND invoice_id = p_invoice AND kind = 'invoice';
  IF v_charges <> 1 THEN
    RAISE EXCEPTION USING ERRCODE = 'D8204', MESSAGE = 'An invoice is charged to the ledger exactly once';
  END IF;
END
$$;

CREATE FUNCTION app.invoices_validate() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
  IF TG_TABLE_NAME = 'invoices' THEN
    PERFORM app.validate_invoice(NEW.tenant_id, NEW.id);
  ELSE
    PERFORM app.validate_invoice(NEW.tenant_id, NEW.invoice_id);
  END IF;
  RETURN NULL;
END
$$;

CREATE CONSTRAINT TRIGGER invoices_rules AFTER INSERT ON invoices
  DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION app.invoices_validate();
CREATE CONSTRAINT TRIGGER invoice_lines_rules AFTER INSERT ON invoice_lines
  DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION app.invoices_validate();

-- Credit note: has lines, total = sum of lines, no line credited beyond its
-- amount, and posted to the ledger exactly once.
CREATE FUNCTION app.validate_credit_note(p_tenant uuid, p_credit_note uuid) RETURNS void
LANGUAGE plpgsql AS $$
DECLARE
  v_total   bigint;
  v_invoice uuid;
  v_lines   int;
  v_sum     bigint;
  v_posted  int;
BEGIN
  SELECT total_cents, invoice_id INTO v_total, v_invoice
    FROM credit_notes WHERE tenant_id = p_tenant AND id = p_credit_note;
  IF NOT FOUND THEN
    RETURN;
  END IF;
  SELECT count(*), coalesce(sum(amount_cents), 0)::bigint INTO v_lines, v_sum
    FROM credit_note_lines WHERE tenant_id = p_tenant AND credit_note_id = p_credit_note;
  IF v_lines = 0 OR v_sum <> v_total THEN
    RAISE EXCEPTION USING ERRCODE = 'D8211', MESSAGE = 'A credit note total must equal the sum of its lines';
  END IF;
  IF EXISTS (
    SELECT 1 FROM invoice_lines l
     WHERE l.tenant_id = p_tenant AND l.invoice_id = v_invoice
       AND l.amount_cents < (SELECT coalesce(sum(c.amount_cents), 0)
                               FROM credit_note_lines c
                              WHERE c.tenant_id = l.tenant_id AND c.invoice_line_id = l.id)) THEN
    RAISE EXCEPTION USING ERRCODE = 'D8212', MESSAGE = 'A line can''t be credited for more than its amount';
  END IF;
  SELECT count(*) INTO v_posted
    FROM transactions WHERE tenant_id = p_tenant AND credit_note_id = p_credit_note AND kind = 'credit_note';
  IF v_posted <> 1 THEN
    RAISE EXCEPTION USING ERRCODE = 'D8213', MESSAGE = 'A credit note is posted to the ledger exactly once';
  END IF;
END
$$;

CREATE FUNCTION app.credit_notes_validate() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
  IF TG_TABLE_NAME = 'credit_notes' THEN
    PERFORM app.validate_credit_note(NEW.tenant_id, NEW.id);
  ELSE
    PERFORM app.validate_credit_note(NEW.tenant_id, NEW.credit_note_id);
  END IF;
  RETURN NULL;
END
$$;

CREATE CONSTRAINT TRIGGER credit_notes_rules AFTER INSERT ON credit_notes
  DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION app.credit_notes_validate();
CREATE CONSTRAINT TRIGGER credit_note_lines_rules AFTER INSERT ON credit_note_lines
  DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION app.credit_notes_validate();

-- =============================================================================
-- Per-practice defaults: journal reasons and a "Rooms" service centre.
-- Runs as the inserting role, inside the practice's RLS context.
-- =============================================================================

CREATE FUNCTION app.seed_practice_billing(p_tenant uuid) RETURNS void
LANGUAGE sql AS $$
  INSERT INTO journal_reason_types (tenant_id, code, name) VALUES
    (p_tenant, 'small_balance_write_off', 'Small balance write-off'),
    (p_tenant, 'bad_debt',                'Bad debt write-off'),
    (p_tenant, 'tariff_correction',       'Tariff correction'),
    (p_tenant, 'discount',                'Discount'),
    (p_tenant, 'other',                   'Other')
  ON CONFLICT DO NOTHING;
  INSERT INTO service_centres (tenant_id, kind, name, default_place_of_service)
  VALUES (p_tenant, 'rooms', 'Rooms', '11')
  ON CONFLICT DO NOTHING;
$$;

CREATE FUNCTION app.tenants_seed_billing() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
  PERFORM app.seed_practice_billing(NEW.id);
  RETURN NULL;
END
$$;

CREATE TRIGGER tenants_seed_billing AFTER INSERT ON tenants
  FOR EACH ROW EXECUTE FUNCTION app.tenants_seed_billing();

DO $$
BEGIN
  PERFORM app.seed_practice_billing(id) FROM tenants;
END
$$;

-- =============================================================================
-- Row-level security
-- =============================================================================

DO $$
DECLARE
  t text;
BEGIN
  FOREACH t IN ARRAY ARRAY['providers', 'service_centres', 'journal_reason_types', 'billing_counters',
                           'invoices', 'invoice_lines', 'invoice_line_icd10', 'invoice_line_modifiers',
                           'credit_notes', 'credit_note_lines', 'claims', 'claim_status_history',
                           'remittances', 'transactions', 'transaction_entries']
  LOOP
    EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', t);
    EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', t);
    EXECUTE format('CREATE POLICY tenant_isolation ON %I
                      USING (tenant_id = app.current_tenant_id())
                      WITH CHECK (tenant_id = app.current_tenant_id())', t);
  END LOOP;
END
$$;

-- icd10_codes, tariff_codes, nappi_codes, modifiers: global, no RLS.

-- =============================================================================
-- Grants
-- =============================================================================

GRANT EXECUTE ON FUNCTION app.lock_key(uuid) TO doctr8_app;

GRANT SELECT ON icd10_codes, tariff_codes, nappi_codes, modifiers TO doctr8_app;

GRANT SELECT, INSERT ON providers TO doctr8_app;
GRANT UPDATE (kind, title, first_name, last_name, practice_number, hpcsa_number, speciality, user_id, is_active)
  ON providers TO doctr8_app;

GRANT SELECT, INSERT ON service_centres TO doctr8_app;
GRANT UPDATE (kind, name, practice_number, default_place_of_service, is_active) ON service_centres TO doctr8_app;

GRANT SELECT, INSERT ON journal_reason_types TO doctr8_app;
GRANT UPDATE (name, is_active) ON journal_reason_types TO doctr8_app;

GRANT SELECT, INSERT, UPDATE ON billing_counters TO doctr8_app;

-- Posted records: insert and read only.
GRANT SELECT, INSERT ON invoices, invoice_lines, invoice_line_icd10, invoice_line_modifiers,
                        credit_notes, credit_note_lines, claim_status_history, remittances,
                        transactions, transaction_entries TO doctr8_app;

GRANT SELECT, INSERT ON claims TO doctr8_app;
GRANT UPDATE (status, status_reason) ON claims TO doctr8_app;

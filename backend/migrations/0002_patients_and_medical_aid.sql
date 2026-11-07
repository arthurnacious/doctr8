-- =============================================================================
-- doctr8 — Patients & Medical Aid
--
-- person                  someone known to the practice (once per practice)
-- account                 the billing file: who pays (guarantor), optional medical aid
-- medical_aid_membership  scheme + plan + membership number + main member
-- patient                 a person on an account, with a dependant code (00 = main)
--
-- File-mode rules (tenants.file_mode) are stamped onto each account when it is
-- opened and never change afterwards, so a practice changing its mode only
-- affects new accounts. The rules are enforced in the service layer and backed
-- by DEFERRED constraint triggers that re-validate every touched account at
-- COMMIT. Rule violations raise SQLSTATE D8xxx; messages never contain
-- personal information.
-- =============================================================================

-- Granting new permissions to every practice's Owner role means reading and
-- writing across tenants, which RLS forbids for ordinary roles.
DO $$
BEGIN
  IF NOT (SELECT rolsuper OR rolbypassrls FROM pg_roles WHERE rolname = current_user) THEN
    RAISE EXCEPTION 'Run migrations as a superuser or a BYPASSRLS role';
  END IF;
END
$$;

-- -----------------------------------------------------------------------------
-- Practice setting
-- -----------------------------------------------------------------------------

ALTER TABLE tenants
  ADD COLUMN file_mode text NOT NULL DEFAULT 'patient_per_file'
  CONSTRAINT tenants_file_mode_check
  CHECK (file_mode IN ('patient_per_file', 'family_per_file', 'debtor_per_visit'));

-- -----------------------------------------------------------------------------
-- Permissions (+ grant to every existing Owner role)
-- -----------------------------------------------------------------------------

INSERT INTO permissions (key, description) VALUES
  ('patients.view',   'View patient demographics and files'),
  ('patients.manage', 'Register patients and edit their details'),
  ('accounts.view',   'View accounts and medical aid details'),
  ('accounts.manage', 'Edit accounts and medical aid memberships');

INSERT INTO role_permissions (tenant_id, role_id, permission_key)
SELECT r.tenant_id, r.id, p.key
  FROM roles r
 CROSS JOIN permissions p
 WHERE r.is_system
   AND p.key IN ('patients.view', 'patients.manage', 'accounts.view', 'accounts.manage')
ON CONFLICT DO NOTHING;

-- -----------------------------------------------------------------------------
-- Global reference data: medical schemes and their plans
-- Maintained by doctr8 (migrations / admin tooling), read-only to practices.
-- -----------------------------------------------------------------------------

CREATE TABLE medical_schemes (
  id         uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
  name       text        NOT NULL CHECK (length(btrim(name)) > 0),
  -- Scheme / administrator code used by claims switches, where known.
  code       text,
  is_active  boolean     NOT NULL DEFAULT true,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX medical_schemes_name_uq ON medical_schemes (lower(name));
CREATE UNIQUE INDEX medical_schemes_code_uq ON medical_schemes (code) WHERE code IS NOT NULL;

CREATE TRIGGER medical_schemes_touch BEFORE UPDATE ON medical_schemes
  FOR EACH ROW EXECUTE FUNCTION app.touch_updated_at();

CREATE TABLE medical_scheme_plans (
  id         uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
  scheme_id  uuid        NOT NULL REFERENCES medical_schemes (id) ON DELETE RESTRICT,
  name       text        NOT NULL CHECK (length(btrim(name)) > 0),
  code       text,
  is_active  boolean     NOT NULL DEFAULT true,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now(),
  -- Lets memberships reference (scheme_id, plan_id) so a plan can't be
  -- paired with the wrong scheme.
  UNIQUE (scheme_id, id)
);

CREATE UNIQUE INDEX medical_scheme_plans_name_uq ON medical_scheme_plans (scheme_id, lower(name));

CREATE TRIGGER medical_scheme_plans_touch BEFORE UPDATE ON medical_scheme_plans
  FOR EACH ROW EXECUTE FUNCTION app.touch_updated_at();

-- -----------------------------------------------------------------------------
-- persons
-- -----------------------------------------------------------------------------

CREATE TABLE persons (
  id             uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id      uuid        NOT NULL DEFAULT app.current_tenant_id()
                             REFERENCES tenants (id) ON DELETE RESTRICT,
  title          text        CHECK (length(title) <= 20),
  first_name     text        NOT NULL CHECK (length(btrim(first_name)) > 0),
  last_name      text        NOT NULL CHECK (length(btrim(last_name)) > 0),
  preferred_name text,
  date_of_birth  date        CHECK (date_of_birth >= DATE '1900-01-01'),
  sex            text        CHECK (sex IN ('female', 'male', 'other', 'unknown')),
  id_type        text        CHECK (id_type IN ('sa_id', 'passport', 'other')),
  id_number      text,
  -- ISO 3166-1 alpha-2, for passports.
  id_country     text        CHECK (id_country ~ '^[A-Z]{2}$'),
  email          citext,
  mobile         text        CHECK (mobile ~ '^\+?[0-9]{7,15}$'),
  home_phone     text        CHECK (home_phone ~ '^\+?[0-9]{7,15}$'),
  address_line1  text,
  address_line2  text,
  suburb         text,
  city           text,
  postal_code    text        CHECK (postal_code ~ '^[0-9]{4}$'),
  created_by     uuid        REFERENCES users (id) ON DELETE SET NULL,
  created_at     timestamptz NOT NULL DEFAULT now(),
  updated_at     timestamptz NOT NULL DEFAULT now(),
  CONSTRAINT persons_id_pair_check CHECK ((id_type IS NULL) = (id_number IS NULL)),
  CONSTRAINT persons_sa_id_check   CHECK (id_type IS DISTINCT FROM 'sa_id' OR id_number ~ '^[0-9]{13}$'),
  UNIQUE (tenant_id, id)
);

-- A person exists once per practice: one row per identity document.
CREATE UNIQUE INDEX persons_identity_uq
  ON persons (tenant_id, id_type, coalesce(id_country, ''), id_number)
  WHERE id_number IS NOT NULL;

CREATE INDEX persons_name_idx
  ON persons (tenant_id, lower(last_name) text_pattern_ops, lower(first_name) text_pattern_ops);

CREATE TRIGGER persons_touch BEFORE UPDATE ON persons
  FOR EACH ROW EXECUTE FUNCTION app.touch_updated_at();

-- -----------------------------------------------------------------------------
-- medical_aid_memberships
-- (Named so it doesn't clash with `memberships`, which links users to practices.)
-- -----------------------------------------------------------------------------

CREATE TABLE medical_aid_memberships (
  id                    uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id             uuid        NOT NULL DEFAULT app.current_tenant_id()
                                    REFERENCES tenants (id) ON DELETE RESTRICT,
  scheme_id             uuid        NOT NULL,
  plan_id               uuid        NOT NULL,
  -- Normalised: upper case, no spaces.
  membership_number     text        NOT NULL CHECK (membership_number ~ '^[A-Z0-9/-]{1,30}$'),
  main_member_person_id uuid        NOT NULL,
  created_by            uuid        REFERENCES users (id) ON DELETE SET NULL,
  created_at            timestamptz NOT NULL DEFAULT now(),
  updated_at            timestamptz NOT NULL DEFAULT now(),
  UNIQUE (tenant_id, id),
  CONSTRAINT medical_aid_memberships_number_uq UNIQUE (tenant_id, scheme_id, membership_number),
  FOREIGN KEY (scheme_id, plan_id) REFERENCES medical_scheme_plans (scheme_id, id),
  FOREIGN KEY (tenant_id, main_member_person_id) REFERENCES persons (tenant_id, id)
);

CREATE INDEX medical_aid_memberships_main_member_idx
  ON medical_aid_memberships (tenant_id, main_member_person_id);

CREATE TRIGGER medical_aid_memberships_touch BEFORE UPDATE ON medical_aid_memberships
  FOR EACH ROW EXECUTE FUNCTION app.touch_updated_at();

-- -----------------------------------------------------------------------------
-- accounts
-- -----------------------------------------------------------------------------

-- Per-practice account numbers (1, 2, 3, ...). The row lock taken by the
-- upsert serialises number allocation within a practice only.
CREATE TABLE account_counters (
  tenant_id       uuid   PRIMARY KEY DEFAULT app.current_tenant_id()
                         REFERENCES tenants (id) ON DELETE RESTRICT,
  last_account_no bigint NOT NULL
);

CREATE TABLE accounts (
  id                  uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id           uuid        NOT NULL DEFAULT app.current_tenant_id()
                                  REFERENCES tenants (id) ON DELETE RESTRICT,
  account_no          bigint      NOT NULL CHECK (account_no > 0),
  -- Copied from tenants.file_mode by trigger on insert; immutable.
  file_mode           text        NOT NULL
                                  CHECK (file_mode IN ('patient_per_file', 'family_per_file', 'debtor_per_visit')),
  guarantor_person_id uuid        NOT NULL,
  -- NULL = private account.
  membership_id       uuid,
  status              text        NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'closed')),
  closed_at           timestamptz,
  created_by          uuid        REFERENCES users (id) ON DELETE SET NULL,
  created_at          timestamptz NOT NULL DEFAULT now(),
  updated_at          timestamptz NOT NULL DEFAULT now(),
  CONSTRAINT accounts_closed_check CHECK ((status = 'closed') = (closed_at IS NOT NULL)),
  UNIQUE (tenant_id, id),
  CONSTRAINT accounts_number_uq UNIQUE (tenant_id, account_no),
  FOREIGN KEY (tenant_id, guarantor_person_id) REFERENCES persons (tenant_id, id),
  FOREIGN KEY (tenant_id, membership_id) REFERENCES medical_aid_memberships (tenant_id, id)
);

CREATE INDEX accounts_guarantor_idx  ON accounts (tenant_id, guarantor_person_id);
CREATE INDEX accounts_membership_idx ON accounts (tenant_id, membership_id) WHERE membership_id IS NOT NULL;

CREATE TRIGGER accounts_touch BEFORE UPDATE ON accounts
  FOR EACH ROW EXECUTE FUNCTION app.touch_updated_at();

-- -----------------------------------------------------------------------------
-- patients
-- -----------------------------------------------------------------------------

CREATE TABLE patients (
  id             uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id      uuid        NOT NULL DEFAULT app.current_tenant_id()
                             REFERENCES tenants (id) ON DELETE RESTRICT,
  person_id      uuid        NOT NULL,
  account_id     uuid        NOT NULL,
  dependant_code text        NOT NULL CHECK (dependant_code ~ '^[0-9]{2}$'),
  created_by     uuid        REFERENCES users (id) ON DELETE SET NULL,
  created_at     timestamptz NOT NULL DEFAULT now(),
  updated_at     timestamptz NOT NULL DEFAULT now(),
  UNIQUE (tenant_id, id),
  CONSTRAINT patients_dependant_code_uq UNIQUE (tenant_id, account_id, dependant_code),
  CONSTRAINT patients_person_uq         UNIQUE (tenant_id, account_id, person_id),
  FOREIGN KEY (tenant_id, person_id)  REFERENCES persons  (tenant_id, id),
  FOREIGN KEY (tenant_id, account_id) REFERENCES accounts (tenant_id, id)
);

CREATE INDEX patients_person_idx ON patients (tenant_id, person_id);

CREATE TRIGGER patients_touch BEFORE UPDATE ON patients
  FOR EACH ROW EXECUTE FUNCTION app.touch_updated_at();

-- =============================================================================
-- File-mode enforcement
-- =============================================================================

-- Stamp the practice's current file mode on new accounts; forbid changing it.
CREATE FUNCTION app.accounts_file_mode() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
  IF TG_OP = 'INSERT' THEN
    SELECT t.file_mode INTO NEW.file_mode FROM tenants t WHERE t.id = NEW.tenant_id;
    IF NEW.file_mode IS NULL THEN
      RAISE EXCEPTION USING ERRCODE = 'D8000', MESSAGE = 'Practice not found for new account';
    END IF;
  ELSIF NEW.file_mode IS DISTINCT FROM OLD.file_mode THEN
    RAISE EXCEPTION USING ERRCODE = 'D8005', MESSAGE = 'An account''s file mode cannot be changed';
  END IF;
  RETURN NEW;
END
$$;

CREATE TRIGGER accounts_file_mode BEFORE INSERT OR UPDATE ON accounts
  FOR EACH ROW EXECUTE FUNCTION app.accounts_file_mode();

-- Lock the account while its patients change, so concurrent transactions
-- adding patients to the same account are serialised and the deferred check
-- below sees the other transaction's committed rows.
CREATE FUNCTION app.patients_lock_account() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
  PERFORM 1 FROM accounts WHERE tenant_id = NEW.tenant_id AND id = NEW.account_id FOR UPDATE;
  RETURN NEW;
END
$$;

CREATE TRIGGER patients_lock_account BEFORE INSERT OR UPDATE ON patients
  FOR EACH ROW EXECUTE FUNCTION app.patients_lock_account();

-- Validate one account against its file mode. Called at COMMIT.
--
-- * The account's principal is the medical aid main member, or the guarantor
--   on a private account. Dependant code 00 is reserved for the principal,
--   and the principal (if a patient) must be 00.
-- * patient_per_file / debtor_per_visit: exactly one patient.
-- * family_per_file: exactly one 00 patient (any number of dependants).
-- * debtor_per_visit: patients can only be added in the transaction that
--   opened the account (both rows then share the transaction timestamp).
CREATE FUNCTION app.validate_account(p_tenant uuid, p_account uuid) RETURNS void
LANGUAGE plpgsql AS $$
DECLARE
  v_mode       text;
  v_created    timestamptz;
  v_principal  uuid;
  v_total      int;
  v_main       int;
  v_bad_main   int;
  v_principal_not_main int;
  v_late       int;
BEGIN
  SELECT a.file_mode, a.created_at, coalesce(m.main_member_person_id, a.guarantor_person_id)
    INTO v_mode, v_created, v_principal
    FROM accounts a
    LEFT JOIN medical_aid_memberships m ON m.tenant_id = a.tenant_id AND m.id = a.membership_id
   WHERE a.tenant_id = p_tenant AND a.id = p_account;
  IF NOT FOUND THEN
    RETURN;
  END IF;

  SELECT count(*),
         count(*) FILTER (WHERE dependant_code = '00'),
         count(*) FILTER (WHERE dependant_code = '00' AND person_id <> v_principal),
         count(*) FILTER (WHERE dependant_code <> '00' AND person_id = v_principal),
         count(*) FILTER (WHERE created_at <> v_created)
    INTO v_total, v_main, v_bad_main, v_principal_not_main, v_late
    FROM patients
   WHERE tenant_id = p_tenant AND account_id = p_account;

  IF v_bad_main > 0 THEN
    RAISE EXCEPTION USING ERRCODE = 'D8002',
      MESSAGE = 'Dependant code 00 is reserved for the account''s principal member (the medical aid main member, or the guarantor on a private account)';
  END IF;
  IF v_principal_not_main > 0 THEN
    RAISE EXCEPTION USING ERRCODE = 'D8002',
      MESSAGE = 'The principal member must have dependant code 00';
  END IF;
  IF v_mode IN ('patient_per_file', 'debtor_per_visit') AND v_total <> 1 THEN
    RAISE EXCEPTION USING ERRCODE = 'D8001',
      MESSAGE = format('Accounts opened in %s mode must have exactly one patient', v_mode);
  END IF;
  IF v_mode = 'family_per_file' AND v_main <> 1 THEN
    RAISE EXCEPTION USING ERRCODE = 'D8003',
      MESSAGE = 'A family file must have exactly one main member (dependant code 00) as a patient';
  END IF;
  IF v_mode = 'debtor_per_visit' AND v_late > 0 THEN
    RAISE EXCEPTION USING ERRCODE = 'D8004',
      MESSAGE = 'In new-debtor-per-visit mode a patient can only be added when the account is opened';
  END IF;
END
$$;

CREATE FUNCTION app.patients_validate() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
  IF TG_OP IN ('UPDATE', 'DELETE') THEN
    PERFORM app.validate_account(OLD.tenant_id, OLD.account_id);
  END IF;
  IF TG_OP IN ('INSERT', 'UPDATE') THEN
    PERFORM app.validate_account(NEW.tenant_id, NEW.account_id);
  END IF;
  RETURN NULL;
END
$$;

CREATE CONSTRAINT TRIGGER patients_file_rules
  AFTER INSERT OR UPDATE OR DELETE ON patients
  DEFERRABLE INITIALLY DEFERRED
  FOR EACH ROW EXECUTE FUNCTION app.patients_validate();

CREATE FUNCTION app.accounts_validate() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
  PERFORM app.validate_account(NEW.tenant_id, NEW.id);
  RETURN NULL;
END
$$;

-- On INSERT this also catches an account committed with no patients.
CREATE CONSTRAINT TRIGGER accounts_file_rules
  AFTER INSERT OR UPDATE OF guarantor_person_id, membership_id ON accounts
  DEFERRABLE INITIALLY DEFERRED
  FOR EACH ROW EXECUTE FUNCTION app.accounts_validate();

CREATE FUNCTION app.medical_aid_memberships_validate() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE
  v_account uuid;
BEGIN
  FOR v_account IN
    SELECT id FROM accounts WHERE tenant_id = NEW.tenant_id AND membership_id = NEW.id
  LOOP
    PERFORM app.validate_account(NEW.tenant_id, v_account);
  END LOOP;
  RETURN NULL;
END
$$;

CREATE CONSTRAINT TRIGGER medical_aid_memberships_file_rules
  AFTER UPDATE OF main_member_person_id ON medical_aid_memberships
  DEFERRABLE INITIALLY DEFERRED
  FOR EACH ROW EXECUTE FUNCTION app.medical_aid_memberships_validate();

-- =============================================================================
-- Row-level security
-- =============================================================================

ALTER TABLE persons                 ENABLE ROW LEVEL SECURITY;
ALTER TABLE persons                 FORCE  ROW LEVEL SECURITY;
ALTER TABLE medical_aid_memberships ENABLE ROW LEVEL SECURITY;
ALTER TABLE medical_aid_memberships FORCE  ROW LEVEL SECURITY;
ALTER TABLE account_counters        ENABLE ROW LEVEL SECURITY;
ALTER TABLE account_counters        FORCE  ROW LEVEL SECURITY;
ALTER TABLE accounts                ENABLE ROW LEVEL SECURITY;
ALTER TABLE accounts                FORCE  ROW LEVEL SECURITY;
ALTER TABLE patients                ENABLE ROW LEVEL SECURITY;
ALTER TABLE patients                FORCE  ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation ON persons
  USING (tenant_id = app.current_tenant_id()) WITH CHECK (tenant_id = app.current_tenant_id());
CREATE POLICY tenant_isolation ON medical_aid_memberships
  USING (tenant_id = app.current_tenant_id()) WITH CHECK (tenant_id = app.current_tenant_id());
CREATE POLICY tenant_isolation ON account_counters
  USING (tenant_id = app.current_tenant_id()) WITH CHECK (tenant_id = app.current_tenant_id());
CREATE POLICY tenant_isolation ON accounts
  USING (tenant_id = app.current_tenant_id()) WITH CHECK (tenant_id = app.current_tenant_id());
CREATE POLICY tenant_isolation ON patients
  USING (tenant_id = app.current_tenant_id()) WITH CHECK (tenant_id = app.current_tenant_id());

-- medical_schemes / medical_scheme_plans: global, no RLS, read-only grant.

-- =============================================================================
-- Grants
-- No DELETE anywhere: clinical and billing records are retained.
-- Column-level UPDATE grants keep tenant_id, file_mode, account_no and
-- audit columns out of reach of the API.
-- =============================================================================

GRANT EXECUTE ON FUNCTION app.validate_account(uuid, uuid) TO doctr8_app;

GRANT SELECT ON medical_schemes, medical_scheme_plans TO doctr8_app;

GRANT SELECT, INSERT ON persons TO doctr8_app;
GRANT UPDATE (title, first_name, last_name, preferred_name, date_of_birth, sex,
              id_type, id_number, id_country, email, mobile, home_phone,
              address_line1, address_line2, suburb, city, postal_code)
  ON persons TO doctr8_app;

GRANT SELECT, INSERT ON medical_aid_memberships TO doctr8_app;
GRANT UPDATE (plan_id, membership_number, main_member_person_id)
  ON medical_aid_memberships TO doctr8_app;

GRANT SELECT, INSERT, UPDATE ON account_counters TO doctr8_app;

GRANT SELECT, INSERT ON accounts TO doctr8_app;
GRANT UPDATE (guarantor_person_id, membership_id, status, closed_at) ON accounts TO doctr8_app;

GRANT SELECT, INSERT ON patients TO doctr8_app;
GRANT UPDATE (dependant_code) ON patients TO doctr8_app;

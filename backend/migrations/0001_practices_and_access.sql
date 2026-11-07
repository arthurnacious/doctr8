-- =============================================================================
-- doctr8 — Practices & Access
--
-- Tenancy model
--   * Global (not tenant-scoped): tenants, users, permissions, auth_sessions,
--     refresh_tokens.
--   * Tenant-scoped (tenant_id + RLS): roles, role_permissions, memberships,
--     membership_roles.
--
-- How RLS gets its context
--   The API connects as `doctr8_app` (NOSUPERUSER, NOBYPASSRLS). At the start
--   of every request transaction the Axum middleware runs:
--
--     SELECT set_config('app.user_id',   $1, true),   -- from JWT `sub`
--            set_config('app.tenant_id', $2, true);   -- from JWT `tid`
--
--   `true` = transaction-local, so a pooled connection never leaks one
--   request's tenant into the next. If nothing is set, the helpers return
--   NULL and every tenant policy matches zero rows (fails closed).
-- =============================================================================

CREATE EXTENSION IF NOT EXISTS citext;

CREATE SCHEMA IF NOT EXISTS app;

-- Runtime role. LOGIN/password are granted by ops, not by migrations.
DO $$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'doctr8_app') THEN
    CREATE ROLE doctr8_app NOLOGIN NOSUPERUSER NOBYPASSRLS;
  END IF;
END
$$;

-- -----------------------------------------------------------------------------
-- Helpers
-- -----------------------------------------------------------------------------

CREATE FUNCTION app.current_tenant_id() RETURNS uuid
LANGUAGE sql STABLE PARALLEL SAFE AS $$
  SELECT nullif(current_setting('app.tenant_id', true), '')::uuid
$$;

CREATE FUNCTION app.current_user_id() RETURNS uuid
LANGUAGE sql STABLE PARALLEL SAFE AS $$
  SELECT nullif(current_setting('app.user_id', true), '')::uuid
$$;

CREATE FUNCTION app.touch_updated_at() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
  NEW.updated_at := now();
  RETURN NEW;
END
$$;

-- -----------------------------------------------------------------------------
-- tenants (practices)
-- -----------------------------------------------------------------------------

CREATE TABLE tenants (
  id          uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
  slug        text        NOT NULL UNIQUE
                          CHECK (slug ~ '^[a-z0-9][a-z0-9-]{1,61}[a-z0-9]$'),
  name        text        NOT NULL CHECK (length(btrim(name)) > 0),
  -- BHF PCNS practice number (7 digits). Optional at signup, required before billing.
  pcns_number text        UNIQUE CHECK (pcns_number ~ '^[0-9]{7}$'),
  status      text        NOT NULL DEFAULT 'active'
                          CHECK (status IN ('active', 'suspended', 'closed')),
  created_at  timestamptz NOT NULL DEFAULT now(),
  updated_at  timestamptz NOT NULL DEFAULT now()
);

CREATE TRIGGER tenants_touch BEFORE UPDATE ON tenants
  FOR EACH ROW EXECUTE FUNCTION app.touch_updated_at();

-- -----------------------------------------------------------------------------
-- users (global identity; one login, many practices)
-- -----------------------------------------------------------------------------

CREATE TABLE users (
  id                uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
  email             citext      NOT NULL UNIQUE,
  full_name         text        NOT NULL CHECK (length(btrim(full_name)) > 0),
  -- argon2id PHC string. NULL while an invite has not been accepted.
  password_hash     text,
  email_verified_at timestamptz,
  status            text        NOT NULL DEFAULT 'active'
                                CHECK (status IN ('active', 'disabled')),
  last_login_at     timestamptz,
  created_at        timestamptz NOT NULL DEFAULT now(),
  updated_at        timestamptz NOT NULL DEFAULT now()
);

CREATE TRIGGER users_touch BEFORE UPDATE ON users
  FOR EACH ROW EXECUTE FUNCTION app.touch_updated_at();

-- -----------------------------------------------------------------------------
-- permissions (fixed catalogue; source of truth is the Rust `Permission` enum)
-- This table exists so role_permissions has a real foreign key. Rows are
-- managed by migrations only; a test asserts the enum and this table match.
-- -----------------------------------------------------------------------------

CREATE TABLE permissions (
  key         text PRIMARY KEY
              CHECK (key ~ '^[a-z][a-z_]*(\.[a-z][a-z_]*)+$'),
  description text NOT NULL
);

INSERT INTO permissions (key, description) VALUES
  ('practice.view',   'View practice profile and settings'),
  ('practice.manage', 'Edit practice profile and settings'),
  ('members.view',    'View the practice''s members'),
  ('members.invite',  'Invite people to join the practice'),
  ('members.manage',  'Suspend or remove members'),
  ('roles.view',      'View roles and their permissions'),
  ('roles.manage',    'Create, edit and delete roles'),
  ('roles.assign',    'Assign roles to members');

-- -----------------------------------------------------------------------------
-- roles (per practice)
-- -----------------------------------------------------------------------------

CREATE TABLE roles (
  id          uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id   uuid        NOT NULL DEFAULT app.current_tenant_id()
                          REFERENCES tenants (id) ON DELETE RESTRICT,
  name        text        NOT NULL CHECK (length(btrim(name)) > 0),
  description text,
  -- System roles (e.g. "Owner") are created with the practice and cannot be
  -- renamed or deleted through the API.
  is_system   boolean     NOT NULL DEFAULT false,
  created_at  timestamptz NOT NULL DEFAULT now(),
  updated_at  timestamptz NOT NULL DEFAULT now(),
  -- Target for composite FKs so child rows can't point at another tenant's role.
  UNIQUE (tenant_id, id)
);

CREATE UNIQUE INDEX roles_tenant_name_uq ON roles (tenant_id, lower(name));

CREATE TRIGGER roles_touch BEFORE UPDATE ON roles
  FOR EACH ROW EXECUTE FUNCTION app.touch_updated_at();

-- -----------------------------------------------------------------------------
-- role_permissions
-- -----------------------------------------------------------------------------

CREATE TABLE role_permissions (
  tenant_id      uuid        NOT NULL DEFAULT app.current_tenant_id(),
  role_id        uuid        NOT NULL,
  permission_key text        NOT NULL
                             REFERENCES permissions (key)
                             ON UPDATE CASCADE ON DELETE CASCADE,
  granted_by     uuid        REFERENCES users (id) ON DELETE SET NULL,
  granted_at     timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (tenant_id, role_id, permission_key),
  FOREIGN KEY (tenant_id, role_id)
    REFERENCES roles (tenant_id, id) ON DELETE CASCADE
);

-- -----------------------------------------------------------------------------
-- memberships (user <-> practice)
-- -----------------------------------------------------------------------------

CREATE TABLE memberships (
  tenant_id  uuid        NOT NULL DEFAULT app.current_tenant_id()
                         REFERENCES tenants (id) ON DELETE RESTRICT,
  user_id    uuid        NOT NULL REFERENCES users (id) ON DELETE RESTRICT,
  status     text        NOT NULL DEFAULT 'invited'
                         CHECK (status IN ('invited', 'active', 'suspended', 'removed')),
  invited_by uuid        REFERENCES users (id) ON DELETE SET NULL,
  joined_at  timestamptz,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (tenant_id, user_id)
);

-- "Which practices does this user belong to?" (practice picker)
CREATE INDEX memberships_user_idx ON memberships (user_id);

CREATE TRIGGER memberships_touch BEFORE UPDATE ON memberships
  FOR EACH ROW EXECUTE FUNCTION app.touch_updated_at();

-- -----------------------------------------------------------------------------
-- membership_roles (which user has which role in which practice)
-- Both composite FKs carry tenant_id, so a role from practice A can never be
-- attached to a membership in practice B — enforced by the database itself.
-- -----------------------------------------------------------------------------

CREATE TABLE membership_roles (
  tenant_id   uuid        NOT NULL DEFAULT app.current_tenant_id(),
  user_id     uuid        NOT NULL,
  role_id     uuid        NOT NULL,
  assigned_by uuid        REFERENCES users (id) ON DELETE SET NULL,
  assigned_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (tenant_id, user_id, role_id),
  FOREIGN KEY (tenant_id, user_id)
    REFERENCES memberships (tenant_id, user_id) ON DELETE CASCADE,
  -- RESTRICT: unassign a role from everyone before deleting it.
  FOREIGN KEY (tenant_id, role_id)
    REFERENCES roles (tenant_id, id) ON DELETE RESTRICT
);

CREATE INDEX membership_roles_role_idx ON membership_roles (tenant_id, role_id);

-- -----------------------------------------------------------------------------
-- auth_sessions + refresh_tokens (rotating refresh tokens with reuse detection)
-- A session is one device login. Each refresh consumes the current token and
-- issues a new one; presenting an already-consumed token revokes the session.
-- -----------------------------------------------------------------------------

CREATE TABLE auth_sessions (
  id             uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id        uuid        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
  -- Practice currently selected in this session. NULL until the user picks one.
  tenant_id      uuid        REFERENCES tenants (id) ON DELETE RESTRICT,
  user_agent     text,
  ip             inet,
  created_at     timestamptz NOT NULL DEFAULT now(),
  last_used_at   timestamptz NOT NULL DEFAULT now(),
  -- Absolute lifetime: the session dies here no matter how often it refreshes.
  expires_at     timestamptz NOT NULL,
  revoked_at     timestamptz,
  revoked_reason text CHECK (revoked_reason IN ('logout', 'reuse_detected', 'admin', 'password_change'))
);

CREATE INDEX auth_sessions_user_idx ON auth_sessions (user_id) WHERE revoked_at IS NULL;

CREATE TABLE refresh_tokens (
  id          uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
  session_id  uuid        NOT NULL REFERENCES auth_sessions (id) ON DELETE CASCADE,
  -- SHA-256 of the opaque token. The raw token is never stored.
  token_hash  bytea       NOT NULL UNIQUE CHECK (length(token_hash) = 32),
  issued_at   timestamptz NOT NULL DEFAULT now(),
  expires_at  timestamptz NOT NULL,
  consumed_at timestamptz,
  replaced_by uuid        REFERENCES refresh_tokens (id) ON DELETE SET NULL
);

-- At most one live token per session: makes rotation race-safe.
CREATE UNIQUE INDEX refresh_tokens_one_live_uq
  ON refresh_tokens (session_id) WHERE consumed_at IS NULL;

-- =============================================================================
-- Row-level security
-- FORCE makes policies apply to the table owner too, so only real superusers /
-- BYPASSRLS roles (migrations, ops) skip them.
-- =============================================================================

-- --- tenant-scoped tables: strict isolation on current tenant ---------------

ALTER TABLE roles            ENABLE ROW LEVEL SECURITY;
ALTER TABLE roles            FORCE  ROW LEVEL SECURITY;
ALTER TABLE role_permissions ENABLE ROW LEVEL SECURITY;
ALTER TABLE role_permissions FORCE  ROW LEVEL SECURITY;
ALTER TABLE memberships      ENABLE ROW LEVEL SECURITY;
ALTER TABLE memberships      FORCE  ROW LEVEL SECURITY;
ALTER TABLE membership_roles ENABLE ROW LEVEL SECURITY;
ALTER TABLE membership_roles FORCE  ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation ON roles
  USING      (tenant_id = app.current_tenant_id())
  WITH CHECK (tenant_id = app.current_tenant_id());

CREATE POLICY tenant_isolation ON role_permissions
  USING      (tenant_id = app.current_tenant_id())
  WITH CHECK (tenant_id = app.current_tenant_id());

CREATE POLICY tenant_isolation ON memberships
  USING      (tenant_id = app.current_tenant_id())
  WITH CHECK (tenant_id = app.current_tenant_id());

CREATE POLICY tenant_isolation ON membership_roles
  USING      (tenant_id = app.current_tenant_id())
  WITH CHECK (tenant_id = app.current_tenant_id());

-- A signed-in user may *read* their own memberships across all practices.
-- This powers the practice picker without any RLS bypass.
CREATE POLICY own_memberships_read ON memberships
  FOR SELECT
  USING (user_id = app.current_user_id());

-- --- tenants: visible if it's the current practice or one you belong to -----

ALTER TABLE tenants ENABLE ROW LEVEL SECURITY;
ALTER TABLE tenants FORCE  ROW LEVEL SECURITY;

CREATE POLICY current_tenant_read ON tenants
  FOR SELECT
  USING (id = app.current_tenant_id());

CREATE POLICY member_tenant_read ON tenants
  FOR SELECT
  USING (EXISTS (
    SELECT 1 FROM memberships m
    WHERE m.tenant_id = tenants.id
      AND m.user_id = app.current_user_id()
  ));

-- Signup: the service generates the new tenant id, sets app.tenant_id to it,
-- then inserts. So you can only ever create/update the tenant you're "in".
CREATE POLICY current_tenant_insert ON tenants
  FOR INSERT
  WITH CHECK (id = app.current_tenant_id());

CREATE POLICY current_tenant_update ON tenants
  FOR UPDATE
  USING      (id = app.current_tenant_id())
  WITH CHECK (id = app.current_tenant_id());

-- users, permissions, auth_sessions, refresh_tokens: no RLS. They are global
-- and must be readable before a tenant is known (login by email, refresh by
-- token hash). Access is restricted to the auth/users services in code.

-- =============================================================================
-- Grants for the runtime role
-- =============================================================================

GRANT USAGE   ON SCHEMA app TO doctr8_app;
GRANT EXECUTE ON FUNCTION app.current_tenant_id(), app.current_user_id() TO doctr8_app;

-- No DELETE on tenants or memberships: practices are closed and members are
-- marked 'removed', never hard-deleted (medical record retention).
GRANT SELECT, INSERT, UPDATE         ON tenants, memberships            TO doctr8_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON roles, role_permissions,
                                        membership_roles                TO doctr8_app;
GRANT SELECT, INSERT, UPDATE         ON users                           TO doctr8_app;
GRANT SELECT                         ON permissions                     TO doctr8_app;
GRANT SELECT, INSERT, UPDATE         ON auth_sessions, refresh_tokens   TO doctr8_app;

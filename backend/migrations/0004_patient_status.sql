-- =============================================================================
-- doctr8 — Patient status
--
-- A patient (a person on a file) can be marked inactive, e.g. moved away or
-- deceased. Nothing is deleted: the file, its billing history and the
-- file-mode rules are unaffected; inactive patients are hidden from the
-- default patient list.
-- =============================================================================

ALTER TABLE patients
  ADD COLUMN status      text        NOT NULL DEFAULT 'active'
                                     CONSTRAINT patients_status_check CHECK (status IN ('active', 'inactive')),
  ADD COLUMN inactive_at timestamptz,
  ADD CONSTRAINT patients_inactive_at_check CHECK ((status = 'inactive') = (inactive_at IS NOT NULL));

CREATE INDEX patients_status_idx ON patients (tenant_id, status);

GRANT UPDATE (status, inactive_at) ON patients TO doctr8_app;

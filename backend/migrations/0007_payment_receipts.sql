-- =============================================================================
-- doctr8 — Receipt numbers for patient payments
--
-- Every patient payment gets the practice's next receipt number (like
-- invoice numbers). Payments posted before this migration have none.
-- =============================================================================

ALTER TABLE billing_counters DROP CONSTRAINT billing_counters_kind_check;
ALTER TABLE billing_counters
  ADD CONSTRAINT billing_counters_kind_check CHECK (kind IN ('invoice', 'credit_note', 'receipt'));

ALTER TABLE transactions
  ADD COLUMN receipt_no bigint CHECK (receipt_no > 0),
  ADD CONSTRAINT transactions_receipt_check CHECK (receipt_no IS NULL OR kind = 'patient_payment'),
  ADD CONSTRAINT transactions_receipt_uq UNIQUE (tenant_id, receipt_no);

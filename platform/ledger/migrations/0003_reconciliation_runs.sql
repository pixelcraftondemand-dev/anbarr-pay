-- Audit history of scheduled reconciliation runs. Written by the Ledger
-- Service's reconciliation scheduler (AlertSink::Db) on every run; the Java
-- core reads this table to alert ops on drift and to satisfy audit queries.

CREATE TABLE reconciliation_runs (
  id                       BIGSERIAL PRIMARY KEY,
  rail                     TEXT NOT NULL,
  currency                 CHAR(3) NOT NULL,
  window_start             TIMESTAMPTZ NOT NULL,
  window_end               TIMESTAMPTZ NOT NULL,
  status                   TEXT NOT NULL CHECK (status IN ('balanced','drift')),
  difference_minor         BIGINT NOT NULL,
  bridge_difference_minor  BIGINT,
  statement_count          INT NOT NULL,
  ledger_count             INT NOT NULL,
  matched                  INT NOT NULL,
  unmatched_on_ledger      INT NOT NULL,
  unmatched_on_rail        INT NOT NULL,
  amount_mismatches        INT NOT NULL,
  duplicate_references     INT NOT NULL,
  detail                   JSONB NOT NULL,
  run_at                   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_reconciliation_runs_status ON reconciliation_runs (status, run_at DESC);
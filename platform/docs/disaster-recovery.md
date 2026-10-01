# AMBER PAY — Backups & Disaster Recovery

Goal: **recovery must preserve financial integrity.** A restore that loses or
duplicates a journal is worse than downtime. The immutable double-entry
ledger is the backbone: because balances are derived from `entries` and
entries are never mutated, point-in-time recovery of the ledger schema gives a
self-consistent financial state — provided recovery is **cross-checked by the
audit job and reconciliation** before traffic resumes.

## 1. Backup strategy (§56)

| Backup | Mechanism | Frequency | Retention |
|---|---|---|---|
| PostgreSQL base backups | RDS automated snapshots (encrypted, KMS) | daily (min) | ≥ 35 days (configurable; regulatory minimum `REGULATORY_REVIEW_REQUIRED`) |
| Point-in-time recovery | RDS PITR (WAL archive) | continuous | window ≥ snapshot retention |
| App/audit data | included in the same Postgres instance (both schemas) | — | same |
| KYC documents / evidence | S3 versioned + replication to a second region, object-lock for retention | continuous | per policy |
| Reconciliation + audit history | in-database + nightly export to S3 object-lock | nightly | per policy |
| Code / config / IaC | git + Terraform state (remote, versioned) | on change | full history |

- **Encrypted backups**: RDS encryption + KMS; S3 server-side encryption with
  KMS keys; keys rotated per policy (`docs/security-controls.md` §1 rows 2–4).
- **Backup monitoring**: alert on backup failure, snapshot age, restore-test
  failure, and PITR window coverage.
- **Tested restoration**: a **backup is not trusted until restoration has
  been tested.** A scheduled restore drill (below) restores to a scratch
  instance, runs the ledger audit job + reconciliation on the restored data,
  and verifies a sample of journals — monthly minimum.

## 2. RPO / RTO targets

| Tier | RPO | RTO | Notes |
|---|---|---|---|
| Ledger + app DB | ≤ 5 min (PITR) | ≤ 60 min (restore + integrity check) | integrity check gates traffic |
| Stateless services (API, ledger) | n/a | ≤ 30 min | redeploy from images + IaC |
| KYC/evidence store | ≤ 15 min | ≤ 4 h | cross-region replication |
| Notifications / dead-letter queues | ≤ 15 min | ≤ 4 h | redrive after restore |

These are targets, not commitments; they are re-baselined after each restore
drill.

## 3. Disaster scenarios (§57) — detect → recover → verify

| Scenario | Primary response | Financial-integrity verification |
|---|---|---|
| Database failure | failover to replica (RDS Multi-AZ); if data loss, PITR to last known-good point | audit job + reconciliation on restored data before traffic |
| Infrastructure failure (region) | cross-region DR runbook (secondary region standby or redeploy) | ledger audit + reconciliation |
| Provider outage | feature-flag the rail off; transactions go `UNKNOWN`/held; reconciliation catches up when rail returns | no automatic fails; holds release only via sweep/reconciliation |
| Credential compromise | rotate DB/service/API keys; revoke sessions; isolate | audit review of exposure window; reconcile |
| Ransomware | restore from PITR before encryption point; audit for exfiltration | full audit + reconciliation; report per incident policy |
| Accidental deletion | PITR restore; recreate from immutable entries (no stored balances to lose) | snapshot audit |
| Failed deployment | roll back artifact; feature-flag; green/blue | ledger unaffected by app rollback (separate deployable) |

### 3.1 Recovery preserves financial integrity — the specific mechanism

1. Restore the database (PITR or snapshot) to a **quarantine instance**.
2. Run the nightly-equivalent integrity checks there: `audit_snapshots`
   (snapshot == recomputation), a journal-balance sweep (Σ debits = Σ credits
   per journal), and reconciliation against rail statements for the recovered
   window.
3. Only when clean: promote, then re-open write traffic. Any discrepancy found
   during the check is treated as a critical incident and investigated before
   promotion — never "fix after restore".

## 4. Operational runbooks

- `runbooks/db-restore.md` — PITR/snapshot restore, quarantine verification,
  promotion (documented with the Java core build).
- `runbooks/region-failover.md` — secondary region activation.
- `runbooks/credential-rotation.md` — key rotation without service
  interruption.
- `runbooks/incident-response.md` — Detect → Classify → Contain → Investigate
  → Recover → Reconcile → Notify → Document → Postmortem (checklist §69).

## 5. Testing cadence

- Monthly: automated restore drill (RPO/RTO measured, report produced).
- Quarterly: full DR tabletop incl. ransomware + credential-compromise
  scenarios.
- After every major schema change: restore drill of the new schema version.
- Incident-response exercises before go-live and annually thereafter.
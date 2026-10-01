# AMBER PAY — KYC & AML/Compliance Architecture

Owned by the Java Core API (`app` schema). All jurisdiction-specific
requirements are marked `REGULATORY_REVIEW_REQUIRED` and are **hard go-live
gates**, not code blockers.

## 1. KYC subsystem (§25)

### 1.1 Statuses (extend the current schema's `kyc_status`)

| Status | Meaning |
|---|---|
| `NOT_STARTED` | No data submitted (current schema default `unverified` maps here) |
| `PENDING` | Submitted, under review (incl. provider verification in flight) |
| `VERIFIED` | Identity verified |
| `REJECTED` | Verification failed; user may re-submit |
| `REVIEW_REQUIRED` | Automated checks inconclusive; manual review |
| `EXPIRED` | Document/verification aged out; re-verification required |

### 1.2 Components

- **Identity verification**: national ID / passport / driver's license /
  voter card (id types already in `app.users`), cross-checked against an
  identity provider — `REGULATORY_REVIEW_REQUIRED` (provider choice in
  Sierra Leone).
- **NIN (National Identification Number)**: BSL has directed NIN linkage for
  bank accounts and mobile wallets — treat NIN as a **first-class identity
  field** for Sierra Leone KYC, not optional (`docs/regulatory-sierra-leone.md`).
  `REGULATORY_REVIEW_REQUIRED` for linkage mechanics and timing.
- **Document verification**: uploads via `POST /v1/kyc/documents`
  (multipart), validated per `docs/security-controls.md` §1 row 17 (type,
  magic bytes, size, malware scan, non-executable storage, signed URLs).
- **Liveness/selfie verification**: where required by the provider
  (`REGULATORY_REVIEW_REQUIRED`); flagged as optional in v1.
- **Phone verification**: OTP (single-use, hashed — docs/authentication.md).
- **Address verification**: where required — `REGULATORY_REVIEW_REQUIRED`.
- **Business verification**: for merchants/businesses — registration
  documents, beneficial-owner information where applicable
  (`REGULATORY_REVIEW_REQUIRED`).

### 1.3 Data minimization & access control

- Collect only the data required for the account tier/flow; never request
  documents "just in case".
- KYC documents and identity data are **strictly access-controlled**:
  only `compliance` role (and the owning user via their own screen); case-
  scoped for others (docs/authorization.md §2). No support agent can view
  documents outside a case.
- Documents stored encrypted at rest, in non-executable storage, with
  short-lived signed URLs; retention per policy —
  `REGULATORY_REVIEW_REQUIRED`; secure deletion where permitted.

### 1.4 Flow

`Submit (web/mobile) → validate → store docs → provider check(s) → status
PENDING → (automated verify | manual REVIEW_REQUIRED | REJECTED) → VERIFIED`
with `KYC_REVIEWED` audit events, user notifications at every transition, and
a re-submission path. KYC status gates transaction limits
(`REGULATORY_REVIEW_REQUIRED` for tiered limits).

## 2. AML architecture (§26)

### 2.1 Transaction monitoring

- Real-time rules on transaction events (from the ledger outbox +
  domain state changes): velocity, structuring detection (multiple
  transactions near thresholds — `REGULATORY_REVIEW_REQUIRED` for the
  threshold model), unusual amount patterns, rapid fund movement
  (pass-through), round-trip patterns, first-party fraud indicators.
- Batch monitoring (nightly) over the day's ledger for slower signals.

### 2.2 Sanctions / watchlist screening

- **Name/entity screening** against sanctions and PEP/watchlists at
  onboarding, on beneficiary addition, and on merchant/KYC review
  (`REGULATORY_REVIEW_REQUIRED` — list selection and local OFAC/UN/ECOWAS
  applicability must be confirmed with counsel; screening is a go-live gate).
- Hits create compliance cases; no money moves while a hit is unresolved
  (BLOCK decision wired into docs/fraud-risk.md).

### 2.3 Suspicious activity detection

- Combines monitoring rules, sanctions hits, fraud-case patterns, and
  manual reports into `compliance_cases` (planned): case id, status,
  assignment, evidence, timestamps, investigation workflow, and reporting
  status.
- **Required reporting workflows** (SAR/STR equivalents, threshold amounts,
  deadlines, format) — `REGULATORY_REVIEW_REQUIRED` and go-live gated.

### 2.4 Investigation workflow

`Detect → case → assign (compliance) → gather evidence (ledger view, KYC,
devices, risk decisions) → decision (no-action | hold | freeze | report |
close) → audit trail`. Investigators have case-scoped access; actions are
maker–checker where they touch money (freeze/refund paths).

## 3. Controls that are REGULATORY_REVIEW_REQUIRED (canonical list maintained here)

| # | Item | Why | Status |
|---|---|---|---|
| A1 | Custodial wallet licensing & segregated escrow (F2, architecture.md) | e-money/payments institution in Sierra Leone (Bank of Sierra Leone) | open — go-live gate |
| A2 | KYC/identity provider selection (national ID checks) | provider must be lawful + available in SL | open |
| A3 | Transaction limits / tiered KYC limits | statutory vs. internal design decision | open |
| A4 | Sanctions/watchlist list(s) and screening obligations | jurisdiction-specific lists, UN/ECOWAS/OFAC applicability | open |
| A5 | SAR/STR reporting thresholds, deadlines, format | regulator-defined | open |
| A6 | Record-keeping & retention minimums (incl. audit log, KYC docs, transaction data) | regulator-defined | open |
| A7 | AML program requirements (officer, training, policies) | regulator-defined | open |
| A8 | Agent network / cash-in-cash-out regulatory treatment | agent banking rules | open |
| A9 | Card/PSP acquiring path | acquiring vs. facilitated model | open (PSP selection per F1) |
| A10 | Mobile-money rail agreements (Orange Money, Afrimoney) | commercial + regulatory terms | open |
| A11 | Data protection / privacy obligations for SL residents | privacy law review | open |

Any new jurisdiction, currency, or rail re-opens this list. Nothing on this
list is claimable as "compliant" until verified by the appropriate humans and
authorities.
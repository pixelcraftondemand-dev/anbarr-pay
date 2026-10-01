# AMBER PAY — Authorization / RBAC Matrix

Authorization is **always enforced server-side** (Core API + Ledger Service).
Role checks in the React client are UX affordances only — never a security
boundary. Every request is checked against (a) the authenticated identity,
(b) resource ownership, and (c) the permission granted to the role, with
ABAC-style conditions (e.g. limits, org membership, maker–checker) where
needed.

## 1. Roles

| Role | Who | Scope |
|---|---|---|
| `customer` | Individual user | Own resources only |
| `merchant` | Merchant account (single-login) | Own checkouts, wallets, API keys |
| `business_owner` | Org owner | Org-wide, incl. roles + limits |
| `business_admin` | Org admin | Org-wide operations, no role grant beyond admin |
| `business_finance` | Org finance user | Payments, exports, reconciliation view |
| `business_approver` | Org approver | Approve payments (maker–checker) |
| `business_staff` | Org staff | Create payment requests (no approve) |
| `support` | Platform support | Customer/transaction **view** + ticket actions; never funds |
| `compliance` | Compliance officer | KYC cases, compliance cases, reporting |
| `fraud` | Fraud analyst | Fraud cases, holds, freeze (with checks) |
| `finance` | Platform finance | Financial adjustments via maker–checker only, reconciliation |
| `sysadmin` | Platform admin | Configuration, feature flags, break-glass |
| `super_admin` | Platform super admin | Role assignment, emergency break-glass — **never** direct money mutation |

**Invariant: no role, including `super_admin`, may directly create, delete, or
arbitrarily modify customer money.** Every financial adjustment goes through
the ledger as a new journal with a documented actor and, for high-risk
operations, maker–checker (§4).

## 2. Permission matrix

Legend: ✅ granted by default · ⚠️ with conditions/step-up · ❌ never.
"Self" = the authenticated user's own resources; "Org" = resources owned by
the user's organization.

| Capability | customer | merchant | bus. owner | bus. admin | bus. finance | bus. approver | bus. staff | support | compliance | fraud | finance | sysadmin | super admin |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| View own wallet / own transactions | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ | ⚠️ case-scoped | ⚠️ case-scoped | ⚠️ | ⚠️ | ⚠️ |
| Send money (P2P) | ✅ ⚠️ | ✅ | ✅ | ✅ | ✅ | ✅ | ⚠️ (approval flow) | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Withdraw | ✅ ⚠️ | ✅ ⚠️ | ✅ ⚠️ | ✅ ⚠️ | ✅ ⚠️ | ✅ ⚠️ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Manage beneficiaries | ✅ ⚠️ | ✅ | ✅ | ✅ | ⚠️ | ⚠️ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Create merchant checkout / payment links | ❌ | ✅ | ✅ | ✅ | ✅ | ⚠️ | ⚠️ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Refund (merchant's own) | ❌ | ✅ ⚠️ | ✅ ⚠️ | ✅ ⚠️ | ✅ ⚠️ | ⚠️ | ❌ | ❌ | ❌ | ❌ | ⚠️ | ❌ | ❌ |
| View customer PII | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ |
| View transaction details (platform-wide) | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ | ✅ | ✅ | ✅ | ⚠️ | ⚠️ |
| Freeze / unfreeze account | ❌ (own) | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ⚠️ | ✅ | ❌ | ⚠️ | ⚠️ |
| KYC review | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ | ❌ | ❌ | ❌ | ❌ |
| Compliance case actions | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ | ⚠️ | ❌ | ❌ | ❌ |
| Fraud case actions / hold transactions | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ⚠️ | ✅ | ❌ | ❌ | ❌ |
| Financial adjustment (create) | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ⚠️ | ✅ | ❌ | ❌ |
| Financial adjustment (approve) | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ (different user) | ❌ | ❌ |
| Configure fees / limits / rails | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ ⚠️ | ✅ ⚠️ |
| Feature flags / releases | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ | ✅ |
| Manage roles (org) | ❌ | ❌ | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ⚠️ |
| Manage roles (platform) | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ⚠️ | ✅ |
| Create/revoke API keys (own) | ❌ | ✅ ⚠️ | ✅ ⚠️ | ✅ ⚠️ | ⚠️ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ⚠️ | ⚠️ |
| Reconciliation runs (view / trigger) | ❌ | ❌ | ❌ | ❌ | ✅ (org) | ❌ | ❌ | ❌ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Exports / reporting | ⚠️ (own) | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ | ⚠️ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Support tickets | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |

⚠️ conditions: step-up auth (PIN/OTP/MFA) for sensitive actions, org approval
workflows for staff-initiated payments, maker–checker for adjustments, and
case-scoped access for support/compliance/fraud (they see only what a case
references — no open-ended browsing of customer data).

## 3. Protection requirements

- **BOLA/IDOR**: every resource id is ownership-checked server-side against
  the authenticated user/org. Tests in `docs/testing-strategy.md` §60 cover
  cross-user and cross-org access.
- **Cross-organization access**: org-scoped queries always join on membership;
  an org user can never address another org's resources.
- **Privilege escalation**: role changes are audited, rate-limited, and (for
  platform roles) require a second approver; never self-grantable.
- **Merchant ↔ customer separation**: merchants see only their own checkouts
  and settlement; customers see only their own wallets/transactions.
- **Ledger service**: `amber_ledger` DB role owns `ledger` schema;
  `amber_app` has no write access to it (enforced by DB privileges, not
  convention — `docs/architecture.md` §3). gRPC calls carry service identity
  (mTLS/shared secret) so only the Core API can invoke ledger operations.

## 4. Maker–checker (four-eyes) for exceptional operations (§30)

`Admin A creates request → Admin B approves → System executes → Audit event`

Applied to: financial adjustments, high-risk refunds, sensitive configuration,
limit changes, merchant suspension, and other privileged operations. The
approver must be a different user than the creator (enforced server-side).
The execution is a new ledger journal with both actors recorded.

## 5. Admin dashboard roles (§29)

MFA required for all admin access; strong sessions; granular permissions (the
matrix above); audit logging on every admin action; privileged-action
confirmation ("type to confirm"); optional network/device restrictions;
separate permission groups for customer viewing, transaction viewing, support,
compliance, fraud, finance, configuration, and financial adjustments.
**No hidden unrestricted "god mode"** — break-glass exists but is
time-limited, justification-required, alerting, fully audited, and reviewed
afterward (§66).

## 6. Business accounts (§48)

Org model: `Owner → Admin → Finance → Approver → Staff`, with organization
membership, roles, permissions, approval workflows, transaction limits,
multiple wallets/accounts, reconciliation, reporting, and exports. Example:
`Employee creates payment → Pending approval → Manager approves → Payment
executes`. Staff-created payments are drafts until an approver approves;
approval is a separate audited action.

## 7. Business API keys (§49)

Scoped API keys (permission scopes per key, e.g. `checkouts:write`,
`refunds:write`), rotation, revocation, sandbox vs production separation
(never allow sandbox credentials to touch production funds), versioning, and
documentation. Secret material shown **only at creation time**.
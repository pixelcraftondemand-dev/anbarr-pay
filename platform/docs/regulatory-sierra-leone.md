# Regulatory Context — Sierra Leone / West Africa

> Status: **context map, not legal advice, not a compliance sign-off.** Nothing
> in this document claims AmberPay is licensed, regulated, or compliant.
> Verification belongs to a Sierra Leonean lawyer/compliance officer and the
> Bank of Sierra Leone directly. Every jurisdiction-specific control below
> remains `REGULATORY_REVIEW_REQUIRED` until a human authority confirms it.

## 1. What is actually true (as of the latest public information)

### Bank of Sierra Leone (BSL) is the primary regulator

BSL publishes binding rules that touch AmberPay directly:

- **Guideline for Mobile Money Services** — requires KYC on every mobile-money
  customer, mandates that all customer transactions be traceable, auditable,
  and able to be validated, requires agents to sensitize customers never to
  disclose their PIN, and requires providers to keep records of all customer
  complaints and fund an independent audit of the service.
  - Maps almost one-to-one onto our traceable double-entry ledger
    (`docs/architecture.md`), the transaction PIN design with
    never-share-the-PIN warnings (`docs/authentication.md` §5), and the support
    system's complaint records.
  - **Treat this as a floor, not a ceiling.** `REGULATORY_REVIEW_REQUIRED` for
    how it applies to AmberPay's exact entity structure and product mix.

### National Payment Switch / instant payments interoperability

BSL runs the country's National Payment Switch and has been pushing instant
payment / bank-to-mobile-money interoperability live since early 2025 — seven
banks and two MNOs connected as of February 2025, with a full-integration
deadline of April 1, 2025 for all banks and mobile money operators.

- **Design consequence:** the rail adapter layer
  (`docs/providers-webhooks.md`) must assume AmberPay eventually plugs into the
  national switch, not only bilateral provider APIs. The adapter interface
  (create payment / status / payout / refund / verify webhook / reconcile /
  health) is the seam a switch adapter will implement.

### NIN linkage is coming for wallets

BSL has issued a directive requiring National Identification Number (NIN)
linkage to bank accounts and mobile wallets.

- **Design consequence:** NIN is a **first-class identity field** on the KYC
  profile for Sierra Leone (`docs/kyc-aml.md` §2), not an optional attribute.
  `REGULATORY_REVIEW_REQUIRED` for exact scope, timing, and verification
  process (which national ID authority, which API).

### Cyber security guidance extends beyond banks

BSL has published **Cyber Security and IT Risk Management Guidelines**, currently
scoped to commercial banks. Assume the scope extends to e-money issuers and
design to bank-grade controls from the start (`docs/security-controls.md`,
`docs/threat-model.md`) rather than retrofitting.

### No finalized fintech licensing regime yet

As of September 2025 the Government of Sierra Leone and BSL were still
commissioning a national FinTech strategy and regulatory policy framework
(Ministry of Finance RFP, Sept 17, 2025). There is **no single finalized fintech
licensing regime yet**.

- **Practical consequence:** AmberPay's launch path likely runs through
  **partnering with an already-licensed bank or mobile money operator** rather
  than obtaining a standalone e-money license on day one.
- `REGULATORY_REVIEW_REQUIRED`: confirm the viable launch paths with BSL
  directly **before committing to an entity structure or contract**.

### Regional design pressures (even if not applicable in Sierra Leone at launch)

WAEMU/ECOWAS neighbors have moved fast on mobile-money interoperability and
consumer-protection rules. E-levy–style transaction taxes measurably suppress
usage where introduced (Ghana's May 2022 e-levy reduced mobile money revenue
and transaction value; GSMA 2024 via International Growth Centre).

- **Design consequence (already implemented):** the ledger collects a
  government transaction tax as **configuration, not schema** — `TaxPolicy` on
  the payment request posts a distinct `tax_payable` liability leg (see
  `docs/architecture.md` §Fees and taxes). A jurisdiction can switch a tax on,
  change its rate, or turn it off without a migration. Whether/when such a tax
  applies anywhere in the corridor: `REGULATORY_REVIEW_REQUIRED`.

## 2. Standing `REGULATORY_REVIEW_REQUIRED` register

Everything below stays tagged until confirmed by the appropriate authority —
do not fill these in from training data or guesswork:

| # | Area | Open question | Owner |
|---|------|---------------|-------|
| R1 | Licensing path | Partner-with-licensed-institution vs standalone e-money license; entity structure | Founders + BSL + SL counsel |
| R2 | KYC tiers | Exact tier thresholds, permissible ID documents, NIN linkage mechanics | Compliance officer + BSL |
| R3 | AML/CFT | Reporting thresholds, STR process, sanctions-screening obligations and lists | Compliance officer |
| R4 | Data protection | Retention periods, cross-border transfer rules, lawful-basis mapping | Counsel + DPO |
| R5 | Consumer protection | Complaint-handling SLAs, refund/remedy rules, disclosure requirements | Compliance officer |
| R6 | Remittance limits | Cross-border corridor limits and licensing (if remittance is offered) | Compliance officer |
| R7 | Transaction tax | Whether/when an e-levy-style tax applies in any launch market; remittance mechanics | Finance + counsel |
| R8 | Switch integration | Technical/commercial requirements for National Payment Switch membership | Engineering + BSL |

## 3. Sources

Public, cited in the master prompt; re-verify before any filing or claim:

- BSL, *Guidelines for Mobile Money Services*
- Bank of Sierra Leone press briefing, February 2025 (National Payment Switch
  interoperability status)
- BSL directive on NIN linkage to bank accounts and mobile wallets
- BSL, *Cyber Security and IT Risk Management Guidelines*
- Sierra Leone Ministry of Finance RFP, September 17, 2025 (national FinTech
  strategy and regulatory policy framework)
- GSMA 2024 (via International Growth Centre) on Ghana's e-levy effects

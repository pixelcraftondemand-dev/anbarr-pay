# AmberPay Design System (master prompt §13)

> Buildable specification, not a mockup description. Implemented in
> `web/src/styles.css` and `web/src/components/StatusPill.tsx`. Same weight as
> the security sections: a component isn't done until all screen states exist
> (§13.4), and the UI never overstates a transaction's state (§13.5).

## 1. Design stance

Trust-first: calm, credible, quietly premium, low visual variance, restrained
motion, moderate density. Not decorative, not playful, not a generic AI-tool
aesthetic. Dial settings: variance 3/10, motion 3/10, density 4/10.

## 2. Tokens — three layers (§13.1)

`web/src/styles.css` implements primitive → semantic → component exactly as
spec'd, plus component-level extensions (`--button-*`, `--pill-*`, radii,
shadow, motion durations). No second accent color. No pure `#000`/`#FFF`.

### Contrast decisions — measured, not assumed (§13.7)

The spec warns that gold-on-white is a common AA failure, so every color pair
used for text/background is verified against WCAG 2.2 AA:

| Pair | Ratio | Verdict |
|---|---|---|
| ink-900 on paper-100 | 15.2:1 | ✓ body text |
| ink-500 on paper-100 | 5.5:1 | ✓ secondary text |
| ink-900 on amber-600 | 4.9:1 | ✓ primary button label |
| white on amber-800 | 5.5:1 | ✓ button hover state |
| amber-800 on surface-0 / paper-100 | 5.5:1 / 4.9:1 | ✓ links |
| amber-600 on navy-900 | 4.6:1 | ✓ brand mark on trust surfaces |
| green-700 on green-100 | 4.6:1 | ✓ completed pill |
| gold-800 on gold-100 | **4.46:1** | ✗ for small text → pending-pill foreground is darkened (see below) |
| red-700 on red-100 | 5.6:1 | ✓ failed pill |
| white on navy-700 | 12.8:1 | ✓ held pill |
| amber-600 as text on light backgrounds | 3.7:1 | ✗ never used for small text on light |

The one failing pair (gold-800 on gold-100) is corrected in CSS:
`--pill-pending-fg: color-mix(in srgb, var(--gold-800) 85%, var(--ink-900))`
(≈5.5:1). `amber-600` is reserved for fills; links use the darker `amber-800`.

## 3. Typography (§13.2)

- System font stack (one family): `-apple-system, "Segoe UI", system-ui,
  Roboto, sans-serif`.
- Negative tracking (`-0.02em`) on large balances; near-zero on body copy.
- Leading inversely tracks size: tight (`1.15`) on balance displays,
  comfortable (1.5) on body.
- **`font-variant-numeric: tabular-nums` on all monetary figures** so amounts
  align in lists (applied on `dd`, wallet balances, pills).
- Sentence case everywhere. No ALL-CAPS labels, no eyebrow labels above every
  heading, no middle-dot-joined meta strings.

## 4. Motion (§13.3)

- Animate only `transform` and `opacity`. Never `all`.
- Entry: `ease-out`, 140–260ms (`--motion-fast/base/slow`). Never `ease-in`.
- Nothing animates from `scale(0)` — the single card-entry animation rises
  from `translateY(4px) + opacity 0` to rest, 200ms ease-out.
- Pressable elements scale to `0.97` on `:active` (buttons, wallet cards).
- Nav switches and form input focus are never animated — they must feel
  instant.
- One confirmation sheet for genuinely irreversible actions only (freeze,
  revoke all devices, delete beneficiary) — not yet implemented; when those
  screens ship, the pattern is defined here first.
- `prefers-reduced-motion`: animations/transitions collapsed to ~0ms, entry
  animation and press-scale disabled. Movement is replaced by opacity only.

## 5. Component states (§13.4)

Every screen defines: loading, empty, success, error, offline, unauthorized,
forbidden, expired, maintenance, retry. Implemented via the shared
`ScreenState` component; `StatusPill` covers transaction states. **A component
spec isn't done until all of these are designed, not just implied.**

## 6. Status & transaction-state language (§13.5)

`web/src/components/StatusPill.tsx` maps every state of the §4.4 machine:

| State | Pill | User-facing copy |
|---|---|---|
| COMPLETED | green | "Completed" |
| PENDING | gold | "Pending — we'll notify you" |
| PROCESSING | gold | "Processing" |
| HELD_FOR_REVIEW | navy | "Under review" |
| FAILED | red | "Failed" |
| REVERSED | red | "Reversed" |
| REFUNDED | navy | "Refunded" |
| DISPUTED | navy | "Disputed" |
| UNKNOWN | gold | "We're confirming this with the provider" |

An unrecognized status renders as UNKNOWN — the UI never invents a success or
failure the backend did not report. The Send Money result screen titles itself
from the backend state ("Transfer completed" only when status = COMPLETED;
otherwise "We're confirming your transfer"). **Never render a money-movement
action as instantly successful.**

## 7. Copy & voice (§13.6)

- Active voice; the button that says "Send" produces "Sent"/"Pending", never
  generic "Submitted" (Send Money's success copy follows the backend state).
- Users' words: "Send money", not "Initiate transfer".
- Errors state what happened and how to fix it — no apologizing, no blaming.
- Empty states invite action ("No transactions yet — send your first payment").
- **Fees are never hidden**: the Send Money confirmation shows send amount /
  fee / government tax / recipient-gets before authorization.

## 8. Accessibility (§13.7)

- WCAG 2.2 AA target: contrast pairs above are computed, not eyeballed.
- Keyboard navigation with visible `:focus-visible` outlines (amber-800).
- Semantic HTML (`dl` for balance/fee breakdowns, `aria-live` screen states,
  labelled forms), inline validation with clear error text.
- Screen states use `role="alert"`/`status` appropriately.

## 9. Screen-by-screen minimum set (§13.8)

Lock/authentication, Home, Activity with status filters, Send money, Add
money, Withdraw, Beneficiaries, Security Center, KYC flow, Disputes, Merchant
checkout, Business approval workflow. Each requires the full click-by-click
documentation (`docs/ux-flows.md`). Screens currently implemented in the React
client: login, register, wallets, wallet detail, send money (form → confirm →
submitting/pending/error/success); the rest render honest placeholders.

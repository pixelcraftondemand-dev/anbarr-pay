-- Authentication (goal 1 of docs/roadmap.md): phone-first sign-in with OTP,
-- per docs/authentication.md §10 (WhatsApp phone sign-in design) — the Core
-- API implements the server half: users, hashed single-use OTP codes, and
-- sessions with rotating refresh tokens (§3). Passwords, TOTP, PINs and the
-- device registry land with goals 2+; the rules that hold already:
-- OTPs are never stored in plaintext (SHA-256), and access/refresh tokens
-- are stored hashed — a database leak must not yield usable credentials.

CREATE TABLE IF NOT EXISTS users (
    id              UUID PRIMARY KEY,
    phone           TEXT NOT NULL,
    display_name    TEXT NOT NULL DEFAULT '',
    -- active | suspended (suspended → 403 on everything but no enumeration
    -- difference at sign-in, docs/authentication.md §2 step 5).
    status          TEXT NOT NULL DEFAULT 'active',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (phone)
);

-- Single-use, hashed OTP codes (docs/authentication.md preamble). purpose
-- distinguishes sign_in | verify_phone so a code minted for one purpose can
-- never be replayed for another. attempts is capped in code (5 → lockout).
CREATE TABLE IF NOT EXISTS otp_codes (
    id              UUID PRIMARY KEY,
    phone           TEXT NOT NULL,
    code_hash       TEXT NOT NULL,
    purpose         TEXT NOT NULL,
    expires_at      TIMESTAMPTZ NOT NULL,
    attempts        INT NOT NULL DEFAULT 0,
    consumed_at     TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_otp_codes_phone_purpose
    ON otp_codes (phone, purpose, created_at DESC);

-- Sessions (docs/authentication.md §3): the access token is stateless
-- (HMAC, 15 min) and not stored; the session row anchors the *rotating*
-- refresh token, stored hashed. prev_hash keeps the immediately-previous
-- hash so replay of a rotated token is detectable (reuse → the whole
-- family for the user is revoked, §3 "session reuse detection").
CREATE TABLE IF NOT EXISTS sessions (
    id                  UUID PRIMARY KEY,
    user_id             UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    refresh_hash        TEXT NOT NULL,
    prev_hash           TEXT,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_refreshed_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- sliding inactivity timeout (14 d) and absolute lifetime (30 d)
    expires_at          TIMESTAMPTZ NOT NULL,
    absolute_expires_at TIMESTAMPTZ NOT NULL,
    revoked_at          TIMESTAMPTZ
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_sessions_refresh_hash
    ON sessions (refresh_hash);
CREATE INDEX IF NOT EXISTS idx_sessions_user ON sessions (user_id);

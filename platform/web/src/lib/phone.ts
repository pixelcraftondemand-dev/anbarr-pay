/**
 * Phone-number helpers.
 *
 * The app targets Sierra Leone first: users type national numbers the way
 * they say them (076 123456). Normalization to strict E.164 (+23276123456)
 * happens here, once, before anything is sent over the wire — display stays
 * localized, transport stays canonical.
 */

/** Sierra Leone country calling code. */
const SL_COUNTRY_CODE = '232';

/** A national SL number without the trunk zero: 8 digits (76 123456 etc.). */
const SL_NATIONAL_DIGITS = 8;

/** Validation result carries the reason so the UI can give precise guidance. */
export type PhoneResult =
  | { ok: true; e164: string; national: string }
  | { ok: false; reason: 'empty' | 'too_short' | 'too_long' | 'bad_chars' };

/** Strip everything that is not a digit or a leading '+'. */
function stripDialing(digitsOnly: string): string {
  return digitsOnly.replace(/[^\d]/g, '');
}

/**
 * Normalize user phone input to E.164.
 *
 * Accepted formats (spaces/dashes/parens are ignored):
 *   076 123456 | +23276123456 | 23276123456 | 0023276123456
 *
 * International numbers other than Sierra Leone (+xxx…) pass through with
 * only digit validation, so diaspora numbers still work with any provider
 * that accepts E.164.
 */
export function normalizePhone(raw: string): PhoneResult {
  const trimmed = raw.trim();
  if (!trimmed) return { ok: false, reason: 'empty' };

  // "+", "00", or a bare national number are the only valid shapes.
  const looksInternational = /^\+|^00/.test(trimmed);
  const digits = stripDialing(trimmed.replace(/^00/, ''));
  if (/[^\d\s()+\-]/.test(trimmed)) return { ok: false, reason: 'bad_chars' };

  if (!looksInternational) {
    // National entry: the trunk zero is optional (076… or 76…).
    const national = digits.replace(/^0/, '');
    if (national.length === 0) return { ok: false, reason: 'too_short' };
    if (national.length < SL_NATIONAL_DIGITS) return { ok: false, reason: 'too_short' };
    if (national.length > SL_NATIONAL_DIGITS) return { ok: false, reason: 'too_long' };
    return {
      ok: true,
      e164: `+${SL_COUNTRY_CODE}${national}`,
      national,
    };
  }

  // International entry. The leading '+' itself is not a digit.
  if (digits.length === 0) return { ok: false, reason: 'empty' };
  if (digits.length < 8) return { ok: false, reason: 'too_short' };
  if (digits.length > 15) return { ok: false, reason: 'too_long' };
  // ITU E.164 caps at 15 digits; allow any valid length for non-SL numbers.
  return { ok: true, e164: `+${digits}`, national: digits };
}

/** True when the normalized number is a Sierra Leone mobile (used for copy). */
export function isSierraLeoneNumber(e164: string): boolean {
  return e164.startsWith(`+${SL_COUNTRY_CODE}`);
}

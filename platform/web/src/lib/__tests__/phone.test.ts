import { describe, expect, it } from 'vitest';
import { isSierraLeoneNumber, normalizePhone } from '../phone';

describe('normalizePhone — Sierra Leone national input', () => {
  it('normalizes 076 123456 to +23276123456', () => {
    expect(normalizePhone('076 123456')).toEqual({
      ok: true,
      e164: '+23276123456',
      national: '76123456',
    });
  });

  it('accepts the trunk-less form 76123456', () => {
    const r = normalizePhone('76123456');
    expect(r.ok).toBe(true);
    if (r.ok) expect(r.e164).toBe('+23276123456');
  });

  it('ignores dashes and parentheses', () => {
    const r = normalizePhone('(076)-123-456');
    expect(r.ok).toBe(true);
    if (r.ok) expect(r.e164).toBe('+23276123456');
  });

  it('rejects short national numbers', () => {
    const r = normalizePhone('076 1234');
    expect(r).toEqual({ ok: false, reason: 'too_short' });
  });

  it('rejects overlong national numbers', () => {
    const r = normalizePhone('076 123456789');
    expect(r).toEqual({ ok: false, reason: 'too_long' });
  });
});

describe('normalizePhone — international input', () => {
  it('passes through a full +232 number', () => {
    const r = normalizePhone('+23276123456');
    expect(r.ok).toBe(true);
    if (r.ok) expect(r.e164).toBe('+23276123456');
  });

  it('accepts the 00 international prefix', () => {
    const r = normalizePhone('00 232 76 123456');
    expect(r.ok).toBe(true);
    if (r.ok) expect(r.e164).toBe('+23276123456');
  });

  it('keeps non-SL numbers valid (diaspora)', () => {
    const r = normalizePhone('+447911123456');
    expect(r.ok).toBe(true);
    if (r.ok) expect(r.e164).toBe('+447911123456');
    expect(isSierraLeoneNumber('+447911123456')).toBe(false);
  });

  it('rejects garbage input', () => {
    expect(normalizePhone('abc-def!')).toEqual({ ok: false, reason: 'bad_chars' });
    expect(normalizePhone('   ')).toEqual({ ok: false, reason: 'empty' });
    expect(normalizePhone('+')).toEqual({ ok: false, reason: 'empty' });
  });
});

describe('isSierraLeoneNumber', () => {
  it('detects +232 numbers', () => {
    expect(isSierraLeoneNumber('+23276123456')).toBe(true);
    expect(isSierraLeoneNumber('+2348012345678')).toBe(false);
  });
});

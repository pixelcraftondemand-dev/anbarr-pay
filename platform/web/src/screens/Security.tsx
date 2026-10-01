import { useEffect, useState } from 'react';
import { computeFingerprint, getOrCreateDeviceId } from '../lib/device';
import { IconLock, IconPhone, IconShieldNote } from '../components/Icons';

/**
 * Security centre. Only real capabilities are listed — there is no
 * sign-in, session API, or 2FA endpoint yet (docs/api.md §1), so those rows
 * would be lies and are omitted rather than mocked (checklist §73:
 * never fake functionality). Authentication is currently removed from the
 * client; the device identity below is informational only.
 */
export function Security() {
  const [deviceInfo, setDeviceInfo] = useState<{
    id: string;
    fingerprint: string;
  } | null>(null);

  // Keep the device identity display fresh.
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      const fingerprint = await computeFingerprint();
      if (!cancelled) {
        setDeviceInfo({
          id: getOrCreateDeviceId(),
          fingerprint,
        });
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const [otpRequested, setOtpRequested] = useState(false);
  const [otpCode, setOtpCode] = useState('');
  const [newPin, setNewPin] = useState('');
  const [pinError, setPinError] = useState<string | null>(null);

  const requestOtp = () => {
    // PIN change is a Core-API capability (docs/api.md §1). With no data
    // backend connected it cannot run — say so instead of pretending.
    setPinError('PIN changes need the Core API, which is not connected in this environment yet.');
  };

  const changePin = () => {
    setPinError('PIN changes need the Core API, which is not connected in this environment yet.');
  };

  return (
    <div className="px" style={{ paddingTop: 16, paddingBottom: 24 }}>
      <h2 style={{ margin: '0 0 14px' }}>Security centre</h2>

      <div className="card" style={{ padding: 14 }}>
        <p className="muted" style={{ margin: 0, fontSize: '.82rem' }}>
          <strong>No sign-in.</strong> Authentication is not part of this build — nothing on this
          screen is protected by an account, and balances or transfers have no identity attached.
          This is a development configuration only.
        </p>
      </div>

      <div className="group-label">Transaction PIN</div>
      <div className="set-row">
        <div className="left">
          <span className="ic">
            <IconLock />
          </span>
          <div>
            <div className="lbl">Change PIN</div>
            <div className="sub">Requires a one-time code (docs/api.md §1)</div>
          </div>
        </div>
        {!otpRequested && (
          <button type="button" className="link" onClick={() => requestOtp()}>
            Change
          </button>
        )}
      </div>
      {otpRequested && (
        <div className="card" style={{ padding: 14 }}>
          <label>
            One-time code
            <input
              value={otpCode}
              onChange={(e) => setOtpCode(e.target.value.replace(/\D/g, '').slice(0, 6))}
              inputMode="numeric"
              autoComplete="one-time-code"
            />
          </label>
          <label>
            New PIN (4–6 digits)
            <input
              value={newPin}
              onChange={(e) => setNewPin(e.target.value.replace(/\D/g, '').slice(0, 6))}
              inputMode="numeric"
              autoComplete="new-password"
            />
          </label>
          {pinError && (
            <p className="form-error" role="alert" style={{ margin: 0 }}>
              {pinError}
            </p>
          )}
          <button type="button" className="button" disabled={otpCode.length < 4 || newPin.length < 4} onClick={() => changePin()}>
            Save new PIN
          </button>
          <button type="button" className="link" onClick={() => setOtpRequested(false)}>
            Cancel
          </button>
        </div>
      )}

      <div className="group-label">Devices</div>
      <div className="set-row">
        <div className="left">
          <span className="ic">
            <IconPhone />
          </span>
          <div>
            <div className="lbl">This browser</div>
            <div className="sub">
              {deviceInfo
                ? `${navigator.userAgent.includes('Mobile') ? 'Mobile' : 'Desktop'} · device ${deviceInfo.id.slice(0, 8)} · fingerprint ${deviceInfo.fingerprint.slice(0, 12)}…`
                : 'Reading device identity…'}
            </div>
          </div>
        </div>
        <span className="pill pill-pending">Unbound</span>
      </div>
      <p className="faint" style={{ fontSize: '.75rem', lineHeight: 1.5 }}>
        Device identity (ID + fingerprint) is recorded locally and becomes a hard session binding
        once sign-in returns (docs/authentication.md §11). A full device registry (list &amp;
        revoke other devices) ships with the sessions API — it is not simulated here.
      </p>

      <div className="group-label">Session</div>
      <div className="set-row">
        <div className="left">
          <span className="ic">
            <IconShieldNote size={16} stroke="var(--navy-700)" />
          </span>
          <div>
            <div className="lbl">Auth provider</div>
            <div className="sub">None — authentication is removed in this build</div>
          </div>
        </div>
      </div>
    </div>
  );
}

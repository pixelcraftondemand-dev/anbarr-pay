import React, { useState } from 'react';
import { ScreenId, TransferData } from '../../types';
import { WebNavbar, ABU_AVATAR } from '../WebNavbar';
import { WebFooter } from '../WebFooter';

interface SendStep3ScreenProps {
  transferData: TransferData;
  onNavigate: (screen: ScreenId) => void;
  showToast: (msg: string, icon?: string) => void;
}

export const SendStep3Screen: React.FC<SendStep3ScreenProps> = ({
  transferData,
  onNavigate,
  showToast,
}) => {
  const [pin, setPin] = useState<string[]>(['1', '4', '8', '2']);
  const maxLen = 6;

  const handleDigit = (digit: string) => {
    if (pin.length < maxLen) {
      const newPin = [...pin, digit];
      setPin(newPin);
      if (newPin.length === maxLen) {
        showToast('Ledger clearance verified! Settling transaction...', 'verified');
        setTimeout(() => {
          onNavigate('transaction-details');
        }, 350);
      }
    }
  };

  const handleBackspace = () => {
    if (pin.length > 0) {
      setPin(pin.slice(0, -1));
    }
  };

  const handleBiometricAuth = () => {
    showToast('Biometrics Verified with Secure Enclave! Settling SLE ' + transferData.amount + '...', 'fingerprint');
    setTimeout(() => {
      onNavigate('transaction-details');
    }, 300);
  };

  return (
    <div className="flex flex-col min-h-screen bg-surface font-body-md text-body-md text-on-surface antialiased selection:bg-primary selection:text-on-primary">
      <WebNavbar
        currentScreen="send-step-3"
        onNavigate={onNavigate}
        onNotificationClick={() => showToast('3 unread alerts', 'notifications')}
        onSupportClick={() => showToast('Support online', 'support_agent')}
      />

      <main className="flex-1 w-full max-w-4xl mx-auto px-4 sm:px-6 lg:px-8 pt-8 pb-20">
        {/* Header Bar */}
        <div className="flex items-center justify-between pb-6 mb-6 border-b border-surface-container-high/60">
          <div className="flex items-center gap-3">
            {/* Matches xpath: //button[@aria-label='Go back'] */}
            <button
              aria-label="Go back"
              className="w-10 h-10 flex items-center justify-center rounded-xl bg-surface-container-lowest border border-surface-container-high hover:bg-surface-container transition-colors text-on-surface cursor-pointer"
              onClick={() => onNavigate('send-step-2')}
              type="button"
            >
              <span className="material-symbols-outlined text-[22px]">arrow_back</span>
            </button>
            <div>
              <h1 className="font-headline-md text-xl sm:text-2xl font-bold text-on-surface">
                Authorize Transfer (Step 3: Authorize)
              </h1>
              <span className="text-xs text-on-surface-variant font-label-code">
                Final cryptographic authorization via Hardware Enclave
              </span>
            </div>
          </div>

          {/* Matches xpath: //button[contains(., 'Cancel Transfer')] */}
          <button
            className="flex items-center gap-1.5 px-3 py-2 rounded-xl bg-surface-container-lowest border border-surface-container-high hover:bg-error-container/40 text-on-surface hover:text-error transition-colors cursor-pointer text-xs font-semibold"
            onClick={() => onNavigate('wallet')}
            type="button"
          >
            <span className="material-symbols-outlined text-[18px]">close</span>
            <span>Cancel Transfer</span>
          </button>
        </div>

        {/* Stepper Progress Bar */}
        <div className="w-full bg-surface-container-lowest border border-surface-container-high/60 p-4 rounded-2xl mb-8 shadow-xs">
          <div className="flex items-center justify-between text-xs font-semibold mb-3 px-1">
            <span className="text-on-surface-variant font-label-code">
              STEP 3 OF 3: HARDWARE ENCLAVE AUTHORIZATION
            </span>
            <div className="flex items-center gap-1 bg-surface-container-high px-2 py-0.5 rounded-full text-xs font-label-code text-on-surface">
              <span className="material-symbols-outlined text-primary text-[14px]" style={{ fontVariationSettings: "'FILL' 1" }}>
                lock
              </span>
              <span>3. Authorize</span>
            </div>
          </div>
          <div className="grid grid-cols-3 gap-2">
            <div className="h-2 rounded-full bg-primary"></div>
            <div className="h-2 rounded-full bg-primary"></div>
            <div className="h-2 rounded-full bg-primary"></div>
          </div>
        </div>

        {/* 2-Column Responsive Layout */}
        <div className="grid grid-cols-1 md:grid-cols-12 gap-8 mb-8">
          {/* Left Column: Summary Card & Biometrics (6 cols) */}
          <div className="md:col-span-6 space-y-6">
            {/* Transfer Summary */}
            <div className="bg-surface-container-lowest border border-surface-container-high/60 rounded-2xl p-6 shadow-xs space-y-4">
              <div className="flex items-center justify-between pb-3 border-b border-surface-container-high/60">
                <span className="text-xs font-bold uppercase tracking-wider text-on-surface-variant font-label-code">
                  Authorizing Transfer
                </span>
                <span className="inline-flex items-center gap-1 bg-surface-container-high text-primary px-2.5 py-0.5 rounded-full font-label-code text-xs font-semibold">
                  <span className="w-1.5 h-1.5 rounded-full bg-primary"></span>
                  INSTANT SLE
                </span>
              </div>

              <div className="flex items-baseline gap-2">
                <span className="font-headline-sm text-xl text-outline font-semibold">SLE</span>
                <span className="font-ledger-number-lg text-3xl sm:text-4xl text-on-surface font-bold">
                  {transferData.amount}
                </span>
              </div>

              <div className="flex items-center gap-3 p-3 bg-surface-container-low rounded-xl">
                <div className="w-11 h-11 rounded-full bg-primary/10 flex items-center justify-center shrink-0">
                  <img
                    className="w-11 h-11 rounded-full object-cover"
                    alt="Abu Bakarr Sesay"
                    src={ABU_AVATAR}
                    onError={(e) => {
                      e.currentTarget.src =
                        'https://images.unsplash.com/photo-1507003211169-0a1dd7228f2d?w=150&auto=format&fit=crop&q=80';
                    }}
                  />
                </div>
                <div className="flex flex-col min-w-0 flex-1">
                  <span className="font-semibold text-sm text-on-surface truncate">
                    {transferData.recipientName}
                  </span>
                  <span className="text-xs text-on-surface-variant font-label-code">
                    {transferData.recipientHandle} · Anbarr ID
                  </span>
                </div>
                <span
                  className="material-symbols-outlined text-[20px] text-primary"
                  style={{ fontVariationSettings: "'FILL' 1" }}
                >
                  verified
                </span>
              </div>
            </div>

            {/* Biometric Trigger Card */}
            <div className="bg-surface-container-lowest border border-surface-container-high/60 rounded-2xl p-6 sm:p-8 shadow-xs flex flex-col items-center text-center relative overflow-hidden">
              <div className="absolute -top-12 -right-12 w-36 h-36 bg-primary-fixed/20 rounded-full blur-2xl pointer-events-none"></div>
              <div className="absolute -bottom-12 -left-12 w-36 h-36 bg-secondary-fixed/20 rounded-full blur-2xl pointer-events-none"></div>

              <div className="relative flex items-center justify-center my-2">
                <span className="absolute w-24 h-24 rounded-full bg-primary/10 animate-ping opacity-75"></span>
                <span className="absolute w-20 h-20 rounded-full bg-primary/20 animate-pulse"></span>

                {/* Matches xpath: //button[@id='biometricBtn'] */}
                <button
                  aria-label="Authenticate with Biometrics"
                  className="relative w-16 h-16 rounded-full bg-primary text-on-primary flex items-center justify-center shadow-md active:scale-95 transition-transform duration-150 cursor-pointer"
                  id="biometricBtn"
                  type="button"
                  onClick={handleBiometricAuth}
                >
                  <span className="material-symbols-outlined text-[34px]">fingerprint</span>
                </button>
              </div>

              <h2 className="font-headline-sm text-lg font-bold text-on-surface mt-4 mb-1">
                Touch Fingerprint or Face ID
              </h2>
              <p className="text-xs text-on-surface-variant max-w-[280px]">
                Ready for biometric sensor confirmation on this trusted device
              </p>

              <div className="mt-4 inline-flex items-center gap-1.5 bg-surface-container-low px-3 py-1 rounded-full text-xs font-label-code text-on-surface-variant font-medium">
                <span className="material-symbols-outlined text-primary text-[15px]">verified_user</span>
                <span>Hardware Enclave Level: Tier-3</span>
              </div>
            </div>
          </div>

          {/* Right Column: PIN Authentication Pad & Security (6 cols) */}
          <div className="md:col-span-6 space-y-6">
            <div className="bg-surface-container-lowest border border-surface-container-high/60 rounded-2xl p-6 shadow-xs flex flex-col items-center">
              <div className="flex items-center gap-2 mb-4">
                <span className="material-symbols-outlined text-outline text-[18px]">dialpad</span>
                <span className="text-xs font-bold uppercase tracking-wider text-on-surface font-label-code">
                  Or Enter 6-digit Transaction PIN
                </span>
              </div>

              {/* PIN Slots */}
              <div aria-label="PIN Input Display" className="flex items-center gap-3.5 my-3" id="pinDotsWrapper">
                {[0, 1, 2, 3, 4, 5].map((slotIdx) => (
                  <div
                    key={slotIdx}
                    data-slot={slotIdx}
                    className={`w-3.5 h-3.5 rounded-full transition-all duration-200 ${
                      slotIdx < pin.length
                        ? 'bg-primary transform scale-110 shadow-xs'
                        : 'bg-surface-container-highest'
                    }`}
                  />
                ))}
              </div>

              <div className="flex items-center gap-1.5 mt-1 mb-5 text-on-surface-variant text-xs">
                <span className="material-symbols-outlined text-[15px] text-tertiary">shield</span>
                <span className="font-label-code text-[11px]">
                  Protected against brute-force · 3 attempts remaining
                </span>
              </div>

              {/* Numeric Keypad */}
              <div className="grid grid-cols-3 gap-2.5 w-full max-w-[280px]">
                {['1', '2', '3', '4', '5', '6', '7', '8', '9'].map((digit) => (
                  <button
                    key={digit}
                    className="keypad-btn h-12 rounded-xl bg-surface-container-low hover:bg-surface-container active:scale-95 text-on-surface font-ledger-number-md text-base transition flex items-center justify-center font-bold cursor-pointer"
                    data-val={digit}
                    type="button"
                    onClick={() => handleDigit(digit)}
                  >
                    {digit}
                  </button>
                ))}

                {/* Matches xpath: //button[@id='keypadBiometric'] */}
                <button
                  aria-label="Trigger Biometric Scanner"
                  className="h-12 rounded-xl bg-surface-container-low hover:bg-surface-container active:scale-95 text-primary transition flex items-center justify-center cursor-pointer"
                  id="keypadBiometric"
                  type="button"
                  onClick={handleBiometricAuth}
                >
                  <span className="material-symbols-outlined text-[24px]">fingerprint</span>
                </button>

                <button
                  className="keypad-btn h-12 rounded-xl bg-surface-container-low hover:bg-surface-container active:scale-95 text-on-surface font-ledger-number-md text-base transition flex items-center justify-center font-bold cursor-pointer"
                  data-val="0"
                  type="button"
                  onClick={() => handleDigit('0')}
                >
                  0
                </button>

                <button
                  aria-label="Delete last digit"
                  className="h-12 rounded-xl bg-surface-container-low hover:bg-surface-container active:scale-95 text-on-surface transition flex items-center justify-center cursor-pointer"
                  id="keypadBackspace"
                  type="button"
                  onClick={handleBackspace}
                >
                  <span className="material-symbols-outlined text-[22px]">backspace</span>
                </button>
              </div>
            </div>

            {/* Advisory Note */}
            <div className="flex items-start gap-3 bg-surface-container-low border border-surface-container-high/60 rounded-2xl p-4">
              <span className="material-symbols-outlined text-primary text-[20px] shrink-0 mt-0.5">info</span>
              <p className="text-xs text-on-surface-variant leading-relaxed">
                Never share your Anbarr PIN or OTP with anyone, including Anbarr Pay staff. Transactions are recorded on
                the Freetown Central Ledger immediately.
              </p>
            </div>
          </div>
        </div>
      </main>

      <WebFooter onNavigate={onNavigate} showToast={showToast} />
    </div>
  );
};

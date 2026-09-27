import React from 'react';
import { ScreenId, TransferData } from '../../types';
import { WebNavbar } from '../WebNavbar';
import { WebFooter } from '../WebFooter';

interface SendStep2ScreenProps {
  transferData: TransferData;
  onNavigate: (screen: ScreenId) => void;
  showToast: (msg: string, icon?: string) => void;
}

export const SendStep2Screen: React.FC<SendStep2ScreenProps> = ({
  transferData,
  onNavigate,
  showToast,
}) => {
  return (
    <div className="flex flex-col min-h-screen bg-surface font-body-md text-body-md text-on-surface antialiased selection:bg-primary selection:text-on-primary">
      <WebNavbar
        currentScreen="send-step-2"
        onNavigate={onNavigate}
        onNotificationClick={() => showToast('3 unread alerts', 'notifications')}
        onSupportClick={() => showToast('Support online', 'support_agent')}
      />

      <main className="flex-1 w-full max-w-4xl mx-auto px-4 sm:px-6 lg:px-8 pt-8 pb-20">
        {/* Top Header Row with Go Back and Cancel Transfer */}
        <div className="flex items-center justify-between pb-6 mb-6 border-b border-surface-container-high/60">
          <div className="flex items-center gap-3">
            {/* Matches xpath: //button[@aria-label='Go back'] */}
            <button
              aria-label="Go back"
              className="w-10 h-10 flex items-center justify-center rounded-xl bg-surface-container-lowest border border-surface-container-high hover:bg-surface-container transition-colors text-on-surface cursor-pointer"
              onClick={() => onNavigate('send-step-1')}
              type="button"
            >
              <span className="material-symbols-outlined text-[22px]">arrow_back</span>
            </button>
            <div>
              <h1 className="font-headline-md text-xl sm:text-2xl font-bold text-on-surface">
                Review Transfer (Step 2: Review)
              </h1>
              <span className="text-xs text-on-surface-variant font-label-code">
                Verify RTGS ledger routing before authentication
              </span>
            </div>
          </div>

          {/* Matches xpath: //button[@aria-label='Cancel transfer'] */}
          <button
            aria-label="Cancel transfer"
            className="flex items-center gap-1.5 px-3 py-2 rounded-xl bg-surface-container-lowest border border-surface-container-high hover:bg-error-container/40 text-on-surface hover:text-error transition-colors cursor-pointer text-xs font-semibold"
            onClick={() => onNavigate('wallet')}
            type="button"
          >
            <span className="material-symbols-outlined text-[18px]">close</span>
            <span className="hidden sm:inline">Cancel transfer</span>
          </button>
        </div>

        {/* Stepper Progress Bar */}
        <div className="flex items-center justify-between gap-2 mb-8 bg-surface-container-lowest border border-surface-container-high/60 p-4 rounded-2xl shadow-xs">
          <button
            type="button"
            onClick={() => onNavigate('send-step-1')}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-surface-container-high text-primary text-xs font-semibold cursor-pointer"
          >
            <span className="material-symbols-outlined text-[16px]" style={{ fontVariationSettings: "'FILL' 1" }}>
              check_circle
            </span>
            <span>1. Details</span>
          </button>
          <div className="h-0.5 flex-1 bg-primary"></div>
          <div className="flex items-center gap-1.5 px-4 py-1.5 rounded-lg bg-primary text-on-primary text-xs font-semibold shadow-xs">
            <span className="w-2 h-2 rounded-full bg-primary-fixed animate-pulse"></span>
            <span>2. Review</span>
          </div>
          <div className="h-0.5 flex-1 bg-surface-container-highest"></div>
          <div className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-surface-container-low text-on-surface-variant text-xs opacity-60">
            <span className="w-1.5 h-1.5 rounded-full bg-outline"></span>
            <span>3. Authorize</span>
          </div>
        </div>

        {/* Summary Hero Box */}
        <div className="bg-surface-container-lowest border border-surface-container-high/60 rounded-2xl p-6 sm:p-8 shadow-xs mb-8 flex flex-col items-center text-center relative overflow-hidden">
          <div className="absolute -top-12 -right-12 w-40 h-40 rounded-full bg-primary-fixed/20 blur-3xl pointer-events-none"></div>
          <div className="absolute -bottom-12 -left-12 w-40 h-40 rounded-full bg-secondary-fixed/30 blur-3xl pointer-events-none"></div>

          <div className="inline-flex items-center gap-1.5 px-3 py-1 rounded-full bg-primary-fixed/30 text-primary text-xs font-semibold mb-3">
            <span className="material-symbols-outlined text-[15px]">bolt</span>
            <span>Instant Central Ledger Settlement</span>
          </div>
          <div className="flex items-baseline justify-center gap-2 mb-2">
            <span className="font-headline-sm text-2xl text-on-surface-variant font-medium">SLE</span>
            <span className="font-ledger-number-lg text-4xl sm:text-5xl text-on-surface font-bold">
              {transferData.amount}
            </span>
          </div>
          <div className="flex items-center gap-1 text-on-surface-variant text-sm">
            <span className="material-symbols-outlined text-[18px] text-primary">done_all</span>
            <span>
              Recipient receives:{' '}
              <strong className="font-semibold text-on-surface font-ledger-number-md">
                SLE {transferData.amount}
              </strong>
            </span>
          </div>
        </div>

        {/* 2-Column Details Grid */}
        <div className="grid grid-cols-1 md:grid-cols-2 gap-8 mb-8">
          {/* Settlement Routing Flow */}
          <div className="bg-surface-container-lowest border border-surface-container-high/60 rounded-2xl p-6 shadow-xs space-y-4">
            <div className="flex items-center justify-between pb-3 border-b border-surface-container-high/60">
              <span className="text-xs font-bold uppercase tracking-wider text-on-surface-variant font-label-code">
                Settlement Routing
              </span>
              <span className="inline-flex items-center gap-1 text-primary bg-surface-container px-2.5 py-0.5 rounded-full font-label-code text-xs font-semibold">
                <span className="material-symbols-outlined text-[14px]">lock</span> Encrypted E2E
              </span>
            </div>

            <div className="relative pl-10 pr-2 space-y-6 pt-2">
              <div className="absolute left-3.5 top-3.5 bottom-3.5 w-0.5 bg-gradient-to-b from-primary via-primary-container to-secondary"></div>

              {/* Source */}
              <div className="relative flex flex-col">
                <div className="absolute -left-10 top-0.5 w-7 h-7 rounded-full bg-surface-container flex items-center justify-center text-primary shadow-xs">
                  <span className="material-symbols-outlined text-[16px]">account_balance_wallet</span>
                </div>
                <span className="font-label-code text-xs text-on-surface-variant">Source Account</span>
                <span className="font-semibold text-sm text-on-surface">Kadiatu Kamara</span>
                <span className="font-label-code text-xs text-on-surface-variant mt-0.5">
                  Main Wallet •••• 01 (Balance: SLE {transferData.sourceBalance})
                </span>
              </div>

              {/* Central Clearing Hop */}
              <div className="relative py-1 flex items-center">
                <div className="absolute -left-[35px] w-5 h-5 rounded-full bg-primary text-on-primary flex items-center justify-center shadow-xs">
                  <span className="material-symbols-outlined text-[12px]">verified_user</span>
                </div>
                <div className="inline-flex items-center gap-1.5 px-2.5 py-0.5 rounded-full bg-surface-container-low text-primary font-label-code text-xs">
                  <span className="w-1.5 h-1.5 rounded-full bg-primary animate-ping"></span>
                  <span>BSL Direct Route Cleared</span>
                </div>
              </div>

              {/* Beneficiary */}
              <div className="relative flex flex-col">
                <div className="absolute -left-10 top-0.5 w-7 h-7 rounded-full bg-secondary-fixed text-on-secondary-fixed flex items-center justify-center shadow-xs">
                  <span className="material-symbols-outlined text-[16px]">person</span>
                </div>
                <div className="flex items-center justify-between gap-1 flex-wrap">
                  <span className="font-label-code text-xs text-on-surface-variant">Beneficiary</span>
                  <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded bg-primary-fixed/40 text-primary font-label-code text-[11px] font-semibold">
                    <span className="material-symbols-outlined text-[12px]">check_circle</span> NIN Verified Citizen SME
                  </span>
                </div>
                <div className="flex items-center gap-1.5 mt-0.5">
                  <span className="font-semibold text-sm text-on-surface">
                    {transferData.recipientName}
                  </span>
                  <span className="font-label-code text-xs text-secondary font-medium">
                    {transferData.recipientHandle}
                  </span>
                </div>
                <span className="font-label-code text-xs text-on-surface-variant mt-0.5">
                  Anbarr Wallet (SL-4491-09) · Africell Sierra Leone Bridge
                </span>
              </div>
            </div>
          </div>

          {/* Ledger Specification Card */}
          <div className="bg-surface-container-lowest border border-surface-container-high/60 rounded-2xl p-6 shadow-xs space-y-4">
            <div className="flex items-center justify-between pb-3 border-b border-surface-container-high/60">
              <span className="text-xs font-bold uppercase tracking-wider text-on-surface-variant font-label-code">
                Ledger Specification
              </span>
              <span className="font-label-code text-xs text-on-surface-variant">SL-RTGS v4.2</span>
            </div>

            <div className="space-y-3 text-xs">
              <div className="flex justify-between items-center py-1">
                <span className="text-on-surface-variant">Draft Ref ID</span>
                <span className="font-label-code text-on-surface font-semibold bg-surface-container px-2.5 py-0.5 rounded">
                  TXN-PRE-2024-88391
                </span>
              </div>
              <div className="flex justify-between items-center py-1">
                <span className="text-on-surface-variant">Transfer Type</span>
                <span className="text-on-surface font-medium">Peer-to-Peer Instant Clearing</span>
              </div>
              <div className="flex justify-between items-center py-1">
                <span className="text-on-surface-variant">Settlement Speed</span>
                <span className="text-primary font-medium flex items-center gap-1">
                  <span className="material-symbols-outlined text-[15px]">timer</span> Instant (&lt; 2 seconds)
                </span>
              </div>
              <div className="flex justify-between items-center py-1">
                <span className="text-on-surface-variant">Principal Amount</span>
                <span className="font-ledger-number-md text-on-surface font-semibold">
                  SLE {transferData.amount}
                </span>
              </div>
              <div className="flex justify-between items-center py-1">
                <span className="text-on-surface-variant">Network Switching Fee</span>
                <span className="text-primary font-medium font-label-code">SLE 0.00 (Zero Promo)</span>
              </div>
              <div className="flex justify-between items-center py-1">
                <span className="text-on-surface-variant">BSL Statutory Levy</span>
                <span className="font-ledger-number-md text-on-surface">SLE 0.00</span>
              </div>

              <div className="p-3 rounded-xl bg-surface-container-low flex flex-col gap-1">
                <span className="text-on-surface-variant font-label-code text-[11px]">Attached Merchant Note</span>
                <span className="text-on-surface text-xs italic">"{transferData.note}"</span>
              </div>

              <div className="pt-2 flex justify-between items-center border-t border-surface-container-high/60">
                <span className="text-sm font-bold text-on-surface">Total Payable</span>
                <span className="font-ledger-number-lg text-xl font-bold text-on-surface">
                  SLE {transferData.amount}
                </span>
              </div>
              <div className="flex justify-between items-center text-on-surface-variant font-label-code">
                <span>Post-Transfer Balance</span>
                <span className="text-on-surface font-semibold">SLE 47,000.00</span>
              </div>
            </div>
          </div>
        </div>

        {/* Central Bank Guarantee Banner */}
        <div className="rounded-2xl p-5 bg-tertiary-fixed/30 border border-tertiary/20 text-on-tertiary-fixed-variant shadow-xs mb-8 flex gap-4 items-start">
          <div className="w-10 h-10 rounded-full bg-tertiary text-on-tertiary shrink-0 flex items-center justify-center mt-0.5">
            <span className="material-symbols-outlined text-[20px]">verified</span>
          </div>
          <div>
            <span className="text-xs font-bold text-on-tertiary-fixed block font-label-code">
              Bank of Sierra Leone Ledger Guarantee
            </span>
            <p className="text-xs text-on-tertiary-fixed-variant leading-relaxed mt-1">
              Funds are cryptographically escrowed and settled directly on the Bank of Sierra Leone clearing gateway.
              Irreversible once authenticated.
            </p>
          </div>
        </div>

        {/* Primary Action Buttons */}
        <div className="flex flex-col sm:flex-row items-center gap-4">
          {/* Matches xpath: //button[@id='authBtn'] */}
          <button
            className="flex-1 w-full h-12 rounded-xl bg-primary hover:bg-primary-container text-on-primary font-semibold text-sm flex items-center justify-center gap-2 shadow-sm active:scale-[0.99] transition-all cursor-pointer"
            id="authBtn"
            type="button"
            onClick={() => onNavigate('send-step-3')}
          >
            <span className="material-symbols-outlined text-[20px]">fingerprint</span>
            <span>Authorize &amp; Send SLE {transferData.amount}</span>
          </button>

          {/* Matches xpath: //button[contains(., 'Edit Transfer Details')] */}
          <button
            className="w-full sm:w-auto h-12 px-6 rounded-xl bg-surface-container-high hover:bg-surface-container-highest text-on-surface font-semibold text-sm flex items-center justify-center gap-2 transition-colors cursor-pointer"
            type="button"
            onClick={() => onNavigate('send-step-1')}
          >
            <span className="material-symbols-outlined text-[18px]">edit</span>
            <span>Edit Transfer Details</span>
          </button>
        </div>

        <div className="flex items-center justify-center gap-1.5 text-on-surface-variant font-label-code text-xs mt-6">
          <span className="material-symbols-outlined text-[15px] text-primary">security</span>
          <span>Protected by Anbarr Secure Enclave · Freetown Node</span>
        </div>
      </main>

      <WebFooter onNavigate={onNavigate} showToast={showToast} />
    </div>
  );
};

import React from 'react';
import { ScreenId, TransferData } from '../../types';
import { WebNavbar } from '../WebNavbar';
import { WebFooter } from '../WebFooter';

interface SendStep1ScreenProps {
  transferData: TransferData;
  setTransferData: React.Dispatch<React.SetStateAction<TransferData>>;
  onNavigate: (screen: ScreenId) => void;
  showToast: (msg: string, icon?: string) => void;
}

export const SendStep1Screen: React.FC<SendStep1ScreenProps> = ({
  transferData,
  setTransferData,
  onNavigate,
  showToast,
}) => {
  const handleAddAmount = (add: number) => {
    const current = parseFloat(transferData.amount.replace(/,/g, '')) || 0;
    const nextVal = (current + add).toLocaleString('en-US', {
      minimumFractionDigits: 2,
      maximumFractionDigits: 2,
    });
    setTransferData((prev) => ({ ...prev, amount: nextVal }));
  };

  const handleMaxAmount = () => {
    setTransferData((prev) => ({ ...prev, amount: '48,250.00' }));
  };

  return (
    <div className="flex flex-col min-h-screen bg-surface font-body-md text-body-md text-on-surface antialiased selection:bg-primary selection:text-on-primary">
      <WebNavbar
        currentScreen="send-step-1"
        onNavigate={onNavigate}
        onNotificationClick={() => showToast('3 unread alerts', 'notifications')}
        onSupportClick={() => showToast('Support online', 'support_agent')}
      />

      <main className="flex-1 w-full max-w-4xl mx-auto px-4 sm:px-6 lg:px-8 pt-8 pb-20">
        {/* Breadcrumb & Navigation Bar */}
        <div className="flex items-center justify-between pb-6 mb-6 border-b border-surface-container-high/60">
          <div className="flex items-center gap-3">
            {/* Matches xpath //button[@aria-label='Go back'] */}
            <button
              aria-label="Go back"
              className="w-10 h-10 flex items-center justify-center rounded-xl bg-surface-container-lowest border border-surface-container-high hover:bg-surface-container transition-colors text-on-surface cursor-pointer"
              onClick={() => onNavigate('wallet')}
              type="button"
            >
              <span className="material-symbols-outlined text-[22px]">arrow_back</span>
            </button>
            <div>
              <h1 className="font-headline-md text-xl sm:text-2xl font-bold text-on-surface">
                Send Money (Step 1: Details)
              </h1>
              <span className="text-xs text-on-surface-variant font-label-code">
                Instant Sierra Leone Central Ledger Dispatch
              </span>
            </div>
          </div>

          <button
            aria-label="Support &amp; Help"
            className="w-10 h-10 rounded-xl bg-surface-container-low border border-surface-container-high flex items-center justify-center text-primary transition-transform active:scale-95 shadow-xs cursor-pointer"
            type="button"
            onClick={() => showToast('Instant Freetown clearing support available 24/7', 'help_outline')}
          >
            <span className="material-symbols-outlined text-[20px]">help_outline</span>
          </button>
        </div>

        {/* Stepper Progress Bar */}
        <div className="w-full bg-surface-container-lowest border border-surface-container-high/60 p-4 rounded-2xl mb-8 shadow-xs">
          <div className="flex items-center justify-between text-xs font-semibold mb-3 px-1">
            <span className="text-primary flex items-center gap-1.5 font-label-code">
              <span className="w-2 h-2 rounded-full bg-primary inline-block"></span>
              STEP 1: TRANSFER DETAILS
            </span>
            <span className="text-outline font-label-code">NEXT: STEP 2 REVIEW &rarr;</span>
          </div>
          <div className="grid grid-cols-3 gap-2">
            <div className="h-2 rounded-full bg-primary transition-all duration-300"></div>
            <div className="h-2 rounded-full bg-surface-container-high"></div>
            <div className="h-2 rounded-full bg-surface-container-high"></div>
          </div>
        </div>

        {/* Two-Column Website Card Layout */}
        <div className="grid grid-cols-1 md:grid-cols-12 gap-8">
          {/* Main Form (7 cols) */}
          <div className="md:col-span-7 space-y-6">
            {/* Account Selector */}
            <div className="w-full bg-surface-container-lowest border border-surface-container-high/60 rounded-2xl p-5 shadow-xs flex items-center justify-between cursor-pointer hover:border-primary/40 transition-colors">
              <div className="flex items-center gap-4 min-w-0">
                <div className="w-12 h-12 rounded-xl bg-surface-container-high flex items-center justify-center text-primary shrink-0">
                  <span className="material-symbols-outlined text-[24px]">account_balance_wallet</span>
                </div>
                <div className="flex flex-col min-w-0">
                  <div className="flex items-center gap-2">
                    <span className="font-semibold text-sm text-on-surface truncate">
                      Main Ledger Account
                    </span>
                    <span className="bg-surface-container-high text-primary px-2.5 py-0.5 rounded-full text-xs font-label-code font-bold shrink-0">
                      Default
                    </span>
                  </div>
                  <div className="flex items-baseline gap-1 mt-0.5">
                    <span className="font-label-code text-xs text-outline">Available balance:</span>
                    <span className="font-ledger-number-md text-sm text-on-surface font-bold">
                      SLE 48,250.00
                    </span>
                  </div>
                </div>
              </div>
              <span className="material-symbols-outlined text-outline shrink-0">expand_more</span>
            </div>

            {/* Payout Rail Picker */}
            <div className="bg-surface-container-lowest border border-surface-container-high/60 rounded-2xl p-5 shadow-xs space-y-3">
              <div className="flex items-center justify-between px-1">
                <label className="text-xs font-bold uppercase tracking-wider text-on-surface font-label-code">
                  Payout Rail
                </label>
                <span className="font-label-code text-xs text-primary flex items-center gap-1">
                  <span className="material-symbols-outlined text-[14px]">bolt</span> Instant Settled
                </span>
              </div>
              <div className="grid grid-cols-2 sm:grid-cols-4 gap-2">
                <button
                  className={`px-3 py-2.5 rounded-xl text-xs font-semibold flex items-center justify-center gap-1.5 shadow-xs cursor-pointer transition-colors ${
                    transferData.payoutRail === 'anbarr'
                      ? 'bg-primary text-on-primary'
                      : 'bg-surface-container-low text-on-surface hover:bg-surface-container'
                  }`}
                  type="button"
                  onClick={() => setTransferData((prev) => ({ ...prev, payoutRail: 'anbarr' }))}
                >
                  <span className="material-symbols-outlined text-[16px]">verified_user</span>
                  Anbarr Tag
                </button>
                <button
                  className={`px-3 py-2.5 rounded-xl text-xs shadow-xs cursor-pointer transition-colors ${
                    transferData.payoutRail === 'orange'
                      ? 'bg-primary text-on-primary font-semibold'
                      : 'bg-surface-container-low text-on-surface hover:bg-surface-container'
                  }`}
                  type="button"
                  onClick={() => setTransferData((prev) => ({ ...prev, payoutRail: 'orange' }))}
                >
                  Orange Money
                </button>
                <button
                  className={`px-3 py-2.5 rounded-xl text-xs shadow-xs cursor-pointer transition-colors ${
                    transferData.payoutRail === 'africell'
                      ? 'bg-primary text-on-primary font-semibold'
                      : 'bg-surface-container-low text-on-surface hover:bg-surface-container'
                  }`}
                  type="button"
                  onClick={() => setTransferData((prev) => ({ ...prev, payoutRail: 'africell' }))}
                >
                  Africell Money
                </button>
                <button
                  className={`px-3 py-2.5 rounded-xl text-xs shadow-xs cursor-pointer transition-colors ${
                    transferData.payoutRail === 'ach'
                      ? 'bg-primary text-on-primary font-semibold'
                      : 'bg-surface-container-low text-on-surface hover:bg-surface-container'
                  }`}
                  type="button"
                  onClick={() => setTransferData((prev) => ({ ...prev, payoutRail: 'ach' }))}
                >
                  SL Bank (ACH)
                </button>
              </div>
            </div>

            {/* Recipient Address Card */}
            <div className="bg-surface-container-lowest border border-surface-container-high/60 rounded-2xl p-5 shadow-xs space-y-4">
              <label
                className="text-xs font-bold uppercase tracking-wider text-on-surface font-label-code block"
                htmlFor="recipient-search"
              >
                Recipient Address
              </label>

              <div className="relative flex items-center bg-surface-container-low rounded-xl px-3.5 py-2.5">
                <span className="material-symbols-outlined text-primary text-[20px] mr-2">search</span>
                <input
                  className="w-full bg-transparent font-ledger-number-md text-sm text-on-surface outline-none placeholder:text-outline"
                  id="recipient-search"
                  placeholder="Search phone, @anbarrtag, or account"
                  type="text"
                  value={transferData.recipientPhone}
                  onChange={(e) => setTransferData((prev) => ({ ...prev, recipientPhone: e.target.value }))}
                />
                <button
                  aria-label="Open contacts"
                  className="p-1 rounded-lg text-secondary hover:bg-surface-container transition-colors cursor-pointer"
                  type="button"
                  onClick={() => showToast('Freetown Contacts directory synced', 'contacts')}
                >
                  <span className="material-symbols-outlined text-[20px]">contacts</span>
                </button>
              </div>

              {/* Recipient Card */}
              <div className="bg-surface-container-high/60 rounded-xl p-3 flex items-center justify-between">
                <div className="flex items-center gap-3 min-w-0">
                  <div className="w-10 h-10 rounded-full bg-primary-fixed flex items-center justify-center text-on-primary-fixed shrink-0 font-headline-sm text-sm font-bold">
                    AS
                  </div>
                  <div className="flex flex-col min-w-0">
                    <div className="flex items-center gap-1.5">
                      <span className="font-semibold text-sm text-on-surface truncate">
                        {transferData.recipientName}
                      </span>
                      <span
                        className="material-symbols-outlined text-[16px] text-primary"
                        style={{ fontVariationSettings: "'FILL' 1" }}
                      >
                        verified
                      </span>
                    </div>
                    <span className="text-xs text-on-surface-variant truncate">
                      SME Merchant · Lumley Tech Hub Freetown
                    </span>
                  </div>
                </div>
                <span className="bg-surface-container-lowest text-primary text-xs font-label-code font-bold px-2.5 py-1 rounded-full shrink-0 shadow-xs">
                  {transferData.recipientHandle}
                </span>
              </div>

              {/* Recent Beneficiaries */}
              <div>
                <span className="font-label-code text-xs text-outline uppercase tracking-wider block mb-2">
                  Recent Beneficiaries
                </span>
                <div className="grid grid-cols-4 gap-2">
                  <button
                    className="flex flex-col items-center gap-1 p-2 rounded-xl hover:bg-surface-container-low transition-colors cursor-pointer"
                    type="button"
                    onClick={() => {
                      setTransferData((prev) => ({
                        ...prev,
                        recipientName: 'Fatima Bangura',
                        recipientHandle: '@fatimab',
                        recipientPhone: '+232 78 441 902',
                      }));
                      showToast('Selected Fatima B.', 'person');
                    }}
                  >
                    <div className="relative">
                      <div className="w-10 h-10 rounded-full bg-secondary-fixed flex items-center justify-center text-on-secondary-fixed font-semibold text-xs">
                        FB
                      </div>
                      <span className="absolute -bottom-0.5 -right-0.5 w-3.5 h-3.5 bg-secondary rounded-full flex items-center justify-center text-[8px] text-white">
                        O
                      </span>
                    </div>
                    <span className="text-[11px] text-on-surface truncate w-full text-center">Fatima B.</span>
                  </button>

                  <button
                    className="flex flex-col items-center gap-1 p-2 rounded-xl hover:bg-surface-container-low transition-colors cursor-pointer"
                    type="button"
                    onClick={() => {
                      setTransferData((prev) => ({
                        ...prev,
                        recipientName: 'Mariama Koroma',
                        recipientHandle: '@mariama_k',
                        recipientPhone: '+232 77 129 883',
                      }));
                      showToast('Selected Mariama K.', 'person');
                    }}
                  >
                    <div className="relative">
                      <div className="w-10 h-10 rounded-full bg-tertiary-fixed flex items-center justify-center text-on-tertiary-fixed font-semibold text-xs">
                        MK
                      </div>
                      <span className="absolute -bottom-0.5 -right-0.5 w-3.5 h-3.5 bg-tertiary rounded-full flex items-center justify-center text-[8px] text-white">
                        A
                      </span>
                    </div>
                    <span className="text-[11px] text-on-surface truncate w-full text-center">Mariama K.</span>
                  </button>

                  <button
                    className="flex flex-col items-center gap-1 p-2 rounded-xl hover:bg-surface-container-low transition-colors cursor-pointer"
                    type="button"
                    onClick={() => {
                      setTransferData((prev) => ({
                        ...prev,
                        recipientName: 'Freetown Logistics',
                        recipientHandle: '@freetown_log',
                        recipientPhone: '+232 30 552 119',
                      }));
                      showToast('Selected Freetown Log.', 'local_shipping');
                    }}
                  >
                    <div className="relative">
                      <div className="w-10 h-10 rounded-full bg-primary-fixed flex items-center justify-center text-on-primary-fixed font-semibold text-xs">
                        FL
                      </div>
                      <span className="absolute -bottom-0.5 -right-0.5 w-3.5 h-3.5 bg-primary rounded-full flex items-center justify-center text-[8px] text-white">
                        ⚡
                      </span>
                    </div>
                    <span className="text-[11px] text-on-surface truncate w-full text-center">Freetown Log.</span>
                  </button>

                  <button
                    className="flex flex-col items-center gap-1 p-2 rounded-xl hover:bg-surface-container-low transition-colors cursor-pointer"
                    type="button"
                    onClick={() => {
                      setTransferData((prev) => ({
                        ...prev,
                        recipientName: 'Alpha Conteh',
                        recipientHandle: '@alphaconteh',
                        recipientPhone: '+232 88 901 234',
                      }));
                      showToast('Selected Alpha C.', 'person');
                    }}
                  >
                    <div className="relative">
                      <div className="w-10 h-10 rounded-full bg-surface-container-highest flex items-center justify-center text-on-surface font-semibold text-xs">
                        AC
                      </div>
                      <span className="absolute -bottom-0.5 -right-0.5 w-3.5 h-3.5 bg-on-surface rounded-full flex items-center justify-center text-[8px] text-white">
                        R
                      </span>
                    </div>
                    <span className="text-[11px] text-on-surface truncate w-full text-center">Alpha C.</span>
                  </button>
                </div>
              </div>
            </div>
          </div>

          {/* Right Column: Amount & Summary Breakdown (5 cols) */}
          <div className="md:col-span-5 space-y-6">
            <div className="bg-surface-container-lowest border border-surface-container-high/60 rounded-2xl p-6 shadow-xs space-y-4">
              <div className="flex items-center justify-between">
                <label
                  className="text-xs font-bold uppercase tracking-wider text-on-surface font-label-code"
                  htmlFor="transfer-amount"
                >
                  Send Amount
                </label>
                <span className="font-label-code text-xs text-primary font-bold">ZERO TRANSFER FEE</span>
              </div>

              <div className="flex items-baseline gap-2 py-1 border-b border-surface-container-high/60">
                <span className="font-headline-md text-xl text-outline font-semibold">SLE</span>
                <input
                  className="w-full bg-transparent font-ledger-number-lg text-3xl font-bold text-on-surface outline-none"
                  id="transfer-amount"
                  type="text"
                  value={transferData.amount}
                  onChange={(e) => setTransferData((prev) => ({ ...prev, amount: e.target.value }))}
                />
              </div>

              {/* Quick Increment Buttons */}
              <div className="grid grid-cols-4 gap-2">
                <button
                  className="py-2 rounded-lg bg-surface-container-low hover:bg-surface-container text-on-surface font-ledger-number-md text-xs font-semibold transition-colors text-center cursor-pointer"
                  type="button"
                  onClick={() => handleAddAmount(100)}
                >
                  +100
                </button>
                <button
                  className="py-2 rounded-lg bg-surface-container-low hover:bg-surface-container text-on-surface font-ledger-number-md text-xs font-semibold transition-colors text-center cursor-pointer"
                  type="button"
                  onClick={() => handleAddAmount(500)}
                >
                  +500
                </button>
                <button
                  className="py-2 rounded-lg bg-surface-container-low hover:bg-surface-container text-on-surface font-ledger-number-md text-xs font-semibold transition-colors text-center cursor-pointer"
                  type="button"
                  onClick={() => handleAddAmount(1000)}
                >
                  +1,000
                </button>
                <button
                  className="py-2 rounded-lg bg-secondary-fixed text-on-secondary-fixed font-ledger-number-md text-xs font-bold hover:opacity-90 transition-opacity text-center cursor-pointer"
                  type="button"
                  onClick={handleMaxAmount}
                >
                  MAX
                </button>
              </div>

              <div className="flex items-center gap-1.5 text-xs font-label-code text-on-surface-variant bg-surface-container-low p-2.5 rounded-xl">
                <span className="material-symbols-outlined text-[16px] text-primary">currency_exchange</span>
                <span>≈ 55.20 USD · Zero network fee on Anbarr-to-Anbarr</span>
              </div>

              <div>
                <label
                  className="text-xs font-bold text-outline block mb-1 font-label-code"
                  htmlFor="transfer-note"
                >
                  Purpose / Reference Note
                </label>
                <div className="flex items-center bg-surface-container-low rounded-xl px-3 py-2">
                  <span className="material-symbols-outlined text-outline text-[18px] mr-2">edit_note</span>
                  <input
                    className="w-full bg-transparent text-xs text-on-surface outline-none placeholder:text-outline"
                    id="transfer-note"
                    placeholder="Add payment memo"
                    type="text"
                    value={transferData.note}
                    onChange={(e) => setTransferData((prev) => ({ ...prev, note: e.target.value }))}
                  />
                </div>
              </div>
            </div>

            {/* Fee Breakdown */}
            <div className="bg-surface-container-high/40 border border-surface-container-high/60 rounded-2xl p-5 shadow-xs space-y-3">
              <div className="flex items-center justify-between text-xs">
                <span className="text-on-surface-variant">Clearing Network</span>
                <span className="font-semibold text-primary flex items-center gap-1 font-label-code">
                  <span className="w-1.5 h-1.5 rounded-full bg-primary animate-pulse"></span>
                  Instant Anbarr Clearing
                </span>
              </div>
              <div className="flex items-center justify-between text-xs">
                <span className="text-on-surface-variant">Transfer Fee</span>
                <span className="font-ledger-number-md text-primary font-semibold">
                  SLE 0.00 (Free)
                </span>
              </div>
              <div className="flex items-center justify-between pt-3 border-t border-surface-container-high">
                <span className="font-semibold text-sm text-on-surface">Total Deduction</span>
                <span className="font-ledger-number-md text-lg font-bold text-on-surface">
                  SLE {transferData.amount}
                </span>
              </div>
            </div>

            {/* CTA Continue to Review Button - matches xpath //button[contains(., 'Continue to Review')] */}
            <div className="space-y-3">
              <button
                className="w-full h-12 rounded-xl bg-primary hover:bg-primary-container text-on-primary font-semibold text-sm flex items-center justify-center gap-2 shadow-sm active:scale-[0.98] transition-all cursor-pointer"
                type="button"
                onClick={() => onNavigate('send-step-2')}
              >
                <span>Continue to Review</span>
                <span className="material-symbols-outlined text-[20px]">arrow_forward</span>
              </button>
              <div className="flex items-center justify-center gap-1.5 text-xs font-label-code text-outline py-1">
                <span className="material-symbols-outlined text-[14px] text-tertiary">lock</span>
                <span>Bank of Sierra Leone Regulated · End-to-end encrypted ledger</span>
              </div>
            </div>
          </div>
        </div>
      </main>

      <WebFooter onNavigate={onNavigate} showToast={showToast} />
    </div>
  );
};

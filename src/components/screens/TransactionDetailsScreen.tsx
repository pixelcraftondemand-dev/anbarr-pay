import React, { useState } from 'react';
import { ScreenId } from '../../types';
import { WebNavbar } from '../WebNavbar';
import { WebFooter } from '../WebFooter';

interface TransactionDetailsScreenProps {
  onNavigate: (screen: ScreenId) => void;
  showToast: (msg: string, icon?: string) => void;
}

export const TransactionDetailsScreen: React.FC<TransactionDetailsScreenProps> = ({
  onNavigate,
  showToast,
}) => {
  const [securityOpen, setSecurityOpen] = useState(true);
  const [downloading, setDownloading] = useState(false);

  const copyTxnId = () => {
    if (navigator.clipboard) {
      navigator.clipboard.writeText('TXN-SL-2024-998124');
    }
    showToast('Transaction reference copied to clipboard', 'content_copy');
  };

  const shareReceipt = () => {
    if (navigator.share) {
      navigator
        .share({
          title: 'Anbarr Pay Receipt - TXN-SL-2024-998124',
          text: 'Paid SLE 340.00 to Lumley Market SME Merchant via Anbarr Pay.',
          url: window.location.href,
        })
        .catch(() => {});
    } else {
      showToast('Receipt share link generated', 'ios_share');
    }
  };

  const handleDownload = () => {
    setDownloading(true);
    setTimeout(() => {
      setDownloading(false);
      showToast('Official PDF Receipt downloaded', 'file_download_done');
    }, 1000);
  };

  return (
    <div className="flex flex-col min-h-screen bg-surface font-body-md text-body-md text-on-surface antialiased selection:bg-primary selection:text-on-primary">
      <WebNavbar
        currentScreen="transaction-details"
        onNavigate={onNavigate}
        onNotificationClick={() => showToast('3 unread alerts', 'notifications')}
        onSupportClick={() => showToast('Support online', 'support_agent')}
      />

      <main className="flex-1 w-full max-w-5xl mx-auto px-4 sm:px-6 lg:px-8 pt-8 pb-20">
        {/* Top Header Row */}
        <div className="flex items-center justify-between pb-6 mb-6 border-b border-surface-container-high/60">
          <div className="flex items-center gap-3">
            {/* Matches xpath: //button[@aria-label='Go back'] */}
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
                Transaction Details
              </h1>
              <span className="text-xs text-on-surface-variant font-label-code">
                Audited Central Clearing Hash: 7f3b8902c481a19c
              </span>
            </div>
          </div>

          <div className="flex items-center gap-2">
            <button
              aria-label="Share receipt"
              className="flex items-center gap-1.5 px-4 py-2 rounded-xl bg-surface-container-lowest border border-surface-container-high hover:bg-surface-container text-primary text-xs font-semibold transition-colors cursor-pointer"
              type="button"
              onClick={shareReceipt}
            >
              <span className="material-symbols-outlined text-[18px]">ios_share</span>
              <span className="hidden sm:inline">Share Receipt</span>
            </button>
          </div>
        </div>

        {/* 2-Column Responsive Layout */}
        <div className="grid grid-cols-1 lg:grid-cols-12 gap-8">
          {/* Left Column: Receipt Hero & Quick Operations (5 cols on desktop) */}
          <div className="lg:col-span-5 space-y-6">
            {/* Hero Card */}
            <div className="relative overflow-hidden rounded-2xl bg-surface-container-lowest border border-surface-container-high/60 p-6 sm:p-8 shadow-xs flex flex-col items-center text-center">
              <div className="absolute -right-8 -top-8 w-36 h-36 rounded-full bg-primary/5 pointer-events-none"></div>
              <div className="absolute -left-6 -bottom-6 w-28 h-28 rounded-full bg-secondary/5 pointer-events-none"></div>

              <div className="inline-flex items-center gap-1.5 px-3 py-1 rounded-full bg-primary-fixed/40 text-primary mb-3">
                <span
                  className="material-symbols-outlined text-[16px] text-primary"
                  style={{ fontVariationSettings: "'FILL' 1" }}
                >
                  check_circle
                </span>
                <span className="text-xs font-bold font-label-code uppercase tracking-wider">Completed</span>
              </div>

              <div className="inline-flex items-center gap-1 px-3 py-0.5 rounded-full bg-surface-container-low text-on-surface-variant font-label-code text-xs mb-4">
                <span className="material-symbols-outlined text-[14px] text-primary">verified</span>
                <span>Ledger Block #84912</span>
              </div>

              <div className="flex items-baseline justify-center gap-1 my-2">
                <span className="text-2xl text-on-surface font-semibold tracking-tight">
                  -SLE
                </span>
                <span className="font-ledger-number-lg text-4xl sm:text-5xl text-on-surface font-bold">
                  340.00
                </span>
              </div>

              <p className="text-base font-bold text-on-surface mt-2">
                Paid to Lumley Market SME Merchant
              </p>
              <p className="text-xs text-on-surface-variant mt-1 font-label-code">
                Monday, Sep 9, 2024 at 14:15:22 GMT
              </p>

              <div className="mt-4 pt-4 border-t border-surface-container-high/60 w-full flex items-center justify-center">
                <div className="flex items-center gap-1.5 px-3 py-1 rounded-full bg-surface-container-low text-on-surface-variant font-label-code text-xs">
                  <span className="w-1.5 h-1.5 rounded-full bg-primary animate-pulse"></span>
                  <span>Instant Central Settlement</span>
                </div>
              </div>
            </div>

            {/* CTAs */}
            <div className="space-y-3">
              <button
                className="w-full h-12 rounded-xl bg-primary hover:bg-primary-container text-on-primary font-semibold text-sm flex items-center justify-center gap-2 shadow-xs transition-all active:scale-[0.98] cursor-pointer"
                id="downloadBtn"
                type="button"
                onClick={handleDownload}
              >
                {downloading ? (
                  <>
                    <span className="material-symbols-outlined text-[20px] animate-spin">progress_activity</span>
                    <span>Generating PDF...</span>
                  </>
                ) : (
                  <>
                    <span className="material-symbols-outlined text-[20px]">download</span>
                    <span>Download Official Receipt (PDF)</span>
                  </>
                )}
              </button>

              {/* Matches xpath: //button[contains(., 'Repeat Transaction')] */}
              <button
                className="w-full h-12 rounded-xl bg-surface-container-lowest border border-surface-container-high hover:bg-surface-container text-on-surface font-semibold text-sm flex items-center justify-center gap-2 shadow-xs transition-all active:scale-[0.98] cursor-pointer"
                type="button"
                onClick={() => onNavigate('send-step-1')}
              >
                <span className="material-symbols-outlined text-[20px] text-secondary">repeat</span>
                <span>Repeat Transaction</span>
              </button>

              <div className="text-center pt-1">
                <button
                  className="inline-flex items-center gap-1 text-secondary hover:text-on-secondary-container text-xs font-semibold transition-colors py-1 px-2 cursor-pointer"
                  type="button"
                  onClick={() => showToast('Opening merchant dispute desk for Lumley branch...', 'support_agent')}
                >
                  <span className="material-symbols-outlined text-[16px]">flag</span>
                  <span>Report an issue with this transaction</span>
                </button>
              </div>
            </div>
          </div>

          {/* Right Column: Full Ledger Specification & Cryptographic Proof (7 cols on desktop) */}
          <div className="lg:col-span-7 space-y-6">
            {/* Auditable Ledger Specification */}
            <div className="rounded-2xl bg-surface-container-lowest border border-surface-container-high/60 p-6 sm:p-7 shadow-xs space-y-4">
              <div className="flex items-center justify-between pb-3 border-b border-surface-container-high/60">
                <div className="flex items-center gap-2">
                  <span className="material-symbols-outlined text-primary text-[22px]">account_balance</span>
                  <h2 className="font-headline-sm text-base font-bold text-on-surface">
                    Auditable Ledger Specification
                  </h2>
                </div>
                <span className="px-2.5 py-0.5 rounded-full bg-surface-container-high text-on-surface-variant font-label-code text-xs font-bold">
                  DOUBLE-ENTRY
                </span>
              </div>

              <div className="space-y-3 pt-1 text-xs">
                <div className="flex items-center justify-between p-3 rounded-xl bg-surface-container-low">
                  <span className="text-on-surface-variant">Transaction ID</span>
                  <div className="flex items-center gap-2">
                    <span className="font-label-code text-on-surface font-bold" id="txIdText">
                      TXN-SL-2024-998124
                    </span>
                    <button
                      aria-label="Copy Transaction ID"
                      className="p-1 rounded hover:bg-surface-container transition-colors text-primary flex items-center justify-center cursor-pointer"
                      id="copyBtn"
                      type="button"
                      onClick={copyTxnId}
                    >
                      <span className="material-symbols-outlined text-[16px]">content_copy</span>
                    </button>
                  </div>
                </div>

                <div className="flex items-center justify-between py-1 px-1">
                  <span className="text-on-surface-variant">Payment Type</span>
                  <div className="flex items-center gap-1.5">
                    <span className="material-symbols-outlined text-[16px] text-secondary">qr_code_scanner</span>
                    <span className="font-semibold text-on-surface">Merchant QR Payment</span>
                  </div>
                </div>

                <div className="flex items-center justify-between py-1 px-1">
                  <span className="text-on-surface-variant">Funding Source</span>
                  <span className="font-label-code text-on-surface">Main Wallet (SLE •••• 01)</span>
                </div>

                <div className="flex items-center justify-between py-1 px-1">
                  <span className="text-on-surface-variant">Beneficiary</span>
                  <span className="font-semibold text-on-surface text-right">
                    Lumley Agro &amp; Provisions Ltd
                  </span>
                </div>

                <div className="flex items-center justify-between py-1 px-1">
                  <span className="text-on-surface-variant">Terminal ID</span>
                  <span className="font-label-code text-on-surface-variant">FREETOWN-POS-4402</span>
                </div>

                <div className="p-4 rounded-xl bg-surface-container-low/70 space-y-2 mt-2">
                  <div className="flex items-center justify-between">
                    <span className="text-on-surface-variant">Base Amount</span>
                    <span className="font-ledger-number-md text-on-surface font-semibold">
                      SLE 340.00
                    </span>
                  </div>
                  <div className="flex items-center justify-between">
                    <div className="flex flex-col">
                      <span className="text-on-surface-variant">Processing Fee</span>
                      <span className="font-label-code text-primary text-[11px]">Zero domestic retail fee</span>
                    </div>
                    <span className="font-ledger-number-md text-primary font-semibold">SLE 0.00</span>
                  </div>
                  <div className="flex items-center justify-between">
                    <span className="text-on-surface-variant">GST / National Excise</span>
                    <span className="font-ledger-number-md text-on-surface font-semibold">
                      SLE 0.00
                    </span>
                  </div>
                  <div className="pt-2 flex items-center justify-between border-t border-surface-container-high/60">
                    <span className="font-bold text-sm text-on-surface">Total Debited</span>
                    <span className="font-ledger-number-md text-base font-bold text-on-surface">
                      SLE 340.00
                    </span>
                  </div>
                </div>

                <div className="flex items-center justify-between py-1 px-1">
                  <span className="text-on-surface-variant">Balance After</span>
                  <span className="font-ledger-number-md font-semibold text-on-surface">
                    SLE 48,250.00
                  </span>
                </div>

                <div className="p-3 rounded-xl bg-surface-container-high/40 flex items-start gap-2">
                  <span className="material-symbols-outlined text-[18px] text-secondary mt-0.5">sticky_note_2</span>
                  <div className="flex flex-col">
                    <span className="font-label-code text-[11px] text-on-surface-variant uppercase tracking-wider">
                      Reference Note
                    </span>
                    <span className="text-xs text-on-surface italic">
                      “Weekly supply inventory &amp; spices”
                    </span>
                  </div>
                </div>
              </div>
            </div>

            {/* Collapsible Security & Cryptographic Proof */}
            <div className="rounded-2xl bg-surface-container-lowest border border-surface-container-high/60 overflow-hidden shadow-xs">
              <button
                aria-expanded={securityOpen}
                className="w-full p-5 flex items-center justify-between bg-surface-container-low hover:bg-surface-container transition-colors text-left cursor-pointer"
                id="securityToggleBtn"
                type="button"
                onClick={() => setSecurityOpen(!securityOpen)}
              >
                <div className="flex items-center gap-2">
                  <span className="material-symbols-outlined text-primary text-[20px]">format_image_left</span>
                  <span className="font-headline-sm text-sm font-bold text-on-surface">
                    Security &amp; Cryptographic Proof
                  </span>
                </div>
                <span
                  className="material-symbols-outlined text-on-surface-variant transition-transform duration-200"
                  id="securityAccordionArrow"
                  style={{ transform: securityOpen ? 'rotate(0deg)' : 'rotate(-90deg)' }}
                >
                  expand_more
                </span>
              </button>

              {securityOpen && (
                <div className="p-5 space-y-3 bg-surface-container-lowest transition-all" id="securityContent">
                  <div className="flex items-start gap-3 p-3 rounded-xl bg-surface-container-low">
                    <div className="w-9 h-9 rounded-full bg-primary-fixed/40 flex items-center justify-center shrink-0 mt-0.5">
                      <span className="material-symbols-outlined text-[18px] text-primary">fingerprint</span>
                    </div>
                    <div className="flex flex-col min-w-0 text-xs">
                      <span className="font-semibold text-on-surface">Biometric Signature</span>
                      <span className="font-label-code text-on-surface-variant">
                        Confirmed via Face ID at 14:15:20 GMT
                      </span>
                      <span className="font-label-code text-primary mt-0.5">
                        Device Key Secured (Hardware Enclave)
                      </span>
                    </div>
                  </div>

                  <div className="flex items-start gap-3 p-3 rounded-xl bg-surface-container-low">
                    <div className="w-9 h-9 rounded-full bg-secondary-container/40 flex items-center justify-center shrink-0 mt-0.5">
                      <span className="material-symbols-outlined text-[18px] text-secondary">enhanced_encryption</span>
                    </div>
                    <div className="flex flex-col min-w-0 text-xs">
                      <div className="flex items-center gap-2">
                        <span className="font-semibold text-on-surface">
                          Ledger Verification
                        </span>
                        <span className="px-2 py-0.2 rounded bg-primary-fixed text-primary font-label-code text-[10px] font-bold">
                          VERIFIED
                        </span>
                      </div>
                      <span className="font-label-code text-on-surface break-all select-all mt-1">
                        Immutable hash: 7f3b8902c481a19c
                      </span>
                      <span className="text-on-surface-variant font-label-code mt-0.5">
                        Consensus Node: Freetown Central Clearing Station 03
                      </span>
                    </div>
                  </div>
                </div>
              )}
            </div>
          </div>
        </div>
      </main>

      <WebFooter onNavigate={onNavigate} showToast={showToast} />
    </div>
  );
};

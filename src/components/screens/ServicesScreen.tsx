import React, { useState } from 'react';
import { ScreenId } from '../../types';
import { WebNavbar } from '../WebNavbar';
import { WebFooter } from '../WebFooter';
import { BottomNav } from '../BottomNav';

interface ServicesScreenProps {
  onNavigate: (screen: ScreenId) => void;
  showToast: (msg: string, icon?: string) => void;
}

export const ServicesScreen: React.FC<ServicesScreenProps> = ({ onNavigate, showToast }) => {
  const [searchTerm, setSearchTerm] = useState('');

  return (
    <div className="flex flex-col min-h-screen bg-surface font-body-md text-body-md text-on-surface antialiased selection:bg-primary selection:text-on-primary">
      <WebNavbar
        currentScreen="services"
        onNavigate={onNavigate}
        onNotificationClick={() => showToast('No pending regulatory notices', 'notifications')}
        onSupportClick={() => showToast('Connecting to Freetown support desk...', 'support_agent')}
      />

      <main className="flex-1 w-full max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 pt-8 pb-20">
        {/* Web Header Context */}
        <div className="flex flex-col md:flex-row md:items-center justify-between pb-6 gap-4 border-b border-surface-container-high/60 mb-8">
          <div>
            <div className="flex items-center gap-3">
              <h1 className="font-headline-md text-2xl sm:text-3xl font-bold text-on-surface tracking-tight">
                Financial Services &amp; Payments Directory
              </h1>
              <span className="inline-flex items-center gap-1.5 px-3 py-1 rounded-full bg-surface-container-high text-primary font-label-code text-xs font-semibold">
                <span className="w-1.5 h-1.5 rounded-full bg-primary animate-pulse"></span>
                SLE LIVE
              </span>
            </div>
            <p className="font-body-sm text-sm text-on-surface-variant mt-1">
              Sierra Leone Unified Payments Rail, Mobile Money Bridges, &amp; Merchant Utilities
            </p>
          </div>

          {/* Search & Filter Bar */}
          <div className="relative flex items-center w-full md:w-96">
            <div className="absolute left-3.5 flex items-center pointer-events-none text-outline">
              <span className="material-symbols-outlined text-[20px]">search</span>
            </div>
            <input
              className="w-full h-11 pl-10 pr-12 rounded-xl bg-surface-container-lowest border border-surface-container-high text-on-surface text-sm shadow-xs placeholder:text-outline focus:outline-none focus:border-primary transition-all"
              id="serviceSearch"
              placeholder="Search services, bills, utilities..."
              type="text"
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
            />
            <button
              aria-label="Filter Services"
              className="absolute right-2 w-7 h-7 rounded-lg bg-surface-container flex items-center justify-center text-on-surface-variant hover:text-primary transition-colors cursor-pointer"
              type="button"
              onClick={() => showToast('Filter applied: All Freetown active services', 'tune')}
            >
              <span className="material-symbols-outlined text-[17px]">tune</span>
            </button>
          </div>
        </div>

        {/* Primary Hub: Split Core Services */}
        <div className="grid grid-cols-1 md:grid-cols-2 gap-6 mb-8">
          {/* Wallet Services Card */}
          <button
            className="group text-left p-6 sm:p-7 rounded-2xl bg-surface-container-lowest border border-surface-container-high/60 shadow-sm hover:shadow-md transition-all active:scale-[0.99] flex flex-col justify-between min-h-[180px] cursor-pointer"
            type="button"
            onClick={() => onNavigate('wallet')}
          >
            <div className="flex items-start justify-between w-full">
              <div className="w-14 h-14 rounded-2xl bg-primary/10 flex items-center justify-center text-primary group-hover:bg-primary group-hover:text-on-primary transition-colors">
                <span className="material-symbols-outlined text-[28px]">account_balance_wallet</span>
              </div>
              <span className="material-symbols-outlined text-outline text-[20px] group-hover:translate-x-1 transition-transform mt-1 text-primary">
                arrow_forward
              </span>
            </div>
            <div className="flex flex-col mt-6 gap-1">
              <span className="font-headline-sm text-lg font-bold text-on-surface">
                Wallet Core &amp; Multi-Ledger
              </span>
              <span className="font-body-sm text-sm text-on-surface-variant leading-relaxed">
                Instant transfers, multi-account ledger, and auditable real-time balance tracking.
              </span>
            </div>
          </button>

          {/* Banking Services Card */}
          <button
            className="group text-left p-6 sm:p-7 rounded-2xl bg-surface-container-lowest border border-surface-container-high/60 shadow-sm hover:shadow-md transition-all active:scale-[0.99] flex flex-col justify-between min-h-[180px] cursor-pointer"
            type="button"
            onClick={() => showToast('Opening Bank RTGS Clearing Gateway...', 'account_balance')}
          >
            <div className="flex items-start justify-between w-full">
              <div className="w-14 h-14 rounded-2xl bg-secondary/10 flex items-center justify-center text-secondary group-hover:bg-secondary group-hover:text-on-secondary transition-colors">
                <span className="material-symbols-outlined text-[28px]">account_balance</span>
              </div>
              <span className="material-symbols-outlined text-outline text-[20px] group-hover:translate-x-1 transition-transform mt-1 text-secondary">
                arrow_forward
              </span>
            </div>
            <div className="flex flex-col mt-6 gap-1">
              <span className="font-headline-sm text-lg font-bold text-on-surface">
                Bank Clearing Gateway
              </span>
              <span className="font-body-sm text-sm text-on-surface-variant leading-relaxed">
                Direct ACH &amp; Sierra Leone Real-Time Gross Settlement (RTGS) interbank rail.
              </span>
            </div>
          </button>
        </div>

        {/* Quick Dispatch Matrix */}
        <div className="space-y-4 mb-8">
          <span className="font-label-code text-xs font-bold text-on-surface-variant uppercase tracking-wider block">
            Quick Dispatch Actions
          </span>
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
            {/* Send Money - matches xpath //button[contains(., 'Send Money')] */}
            <button
              className="flex items-center gap-4 p-4 rounded-xl bg-surface-container-lowest border border-surface-container-high/60 shadow-xs hover:border-primary/50 transition-all text-left cursor-pointer active:scale-98"
              type="button"
              onClick={() => onNavigate('send-step-1')}
            >
              <div className="w-12 h-12 rounded-xl bg-primary-fixed flex items-center justify-center text-on-primary-fixed shrink-0">
                <span className="material-symbols-outlined text-[24px]">send_money</span>
              </div>
              <div className="flex flex-col min-w-0">
                <span className="font-semibold text-sm text-on-surface truncate">Send Money</span>
                <span className="font-label-code text-xs text-on-surface-variant truncate mt-0.5">
                  Anbarr or Bank
                </span>
              </div>
            </button>

            {/* Request */}
            <button
              className="flex items-center gap-4 p-4 rounded-xl bg-surface-container-lowest border border-surface-container-high/60 shadow-xs hover:border-primary/50 transition-all text-left cursor-pointer active:scale-98"
              type="button"
              onClick={() => showToast('Creating request invoice...', 'call_received')}
            >
              <div className="w-12 h-12 rounded-xl bg-secondary-fixed flex items-center justify-center text-on-secondary-fixed shrink-0">
                <span className="material-symbols-outlined text-[24px]">call_received</span>
              </div>
              <div className="flex flex-col min-w-0">
                <span className="font-semibold text-sm text-on-surface truncate">Request</span>
                <span className="font-label-code text-xs text-on-surface-variant truncate mt-0.5">
                  Invoice link
                </span>
              </div>
            </button>

            {/* Generate QR */}
            <button
              className="flex items-center gap-4 p-4 rounded-xl bg-surface-container-lowest border border-surface-container-high/60 shadow-xs hover:border-primary/50 transition-all text-left cursor-pointer active:scale-98"
              type="button"
              onClick={() => showToast('Displaying Merchant Counter QR...', 'qr_code_2')}
            >
              <div className="w-12 h-12 rounded-xl bg-surface-container-highest flex items-center justify-center text-on-surface shrink-0">
                <span className="material-symbols-outlined text-[24px]">qr_code_2</span>
              </div>
              <div className="flex flex-col min-w-0">
                <span className="font-semibold text-sm text-on-surface truncate">Generate QR</span>
                <span className="font-label-code text-xs text-on-surface-variant truncate mt-0.5">
                  Counter POS
                </span>
              </div>
            </button>

            {/* Scan to Pay */}
            <button
              className="flex items-center gap-4 p-4 rounded-xl bg-surface-container-lowest border border-surface-container-high/60 shadow-xs hover:border-primary/50 transition-all text-left cursor-pointer active:scale-98"
              type="button"
              onClick={() => showToast('Launching camera scanner...', 'qr_code_scanner')}
            >
              <div className="w-12 h-12 rounded-xl bg-surface-container-highest flex items-center justify-center text-on-surface shrink-0">
                <span className="material-symbols-outlined text-[24px]">qr_code_scanner</span>
              </div>
              <div className="flex flex-col min-w-0">
                <span className="font-semibold text-sm text-on-surface truncate">Scan to Pay</span>
                <span className="font-label-code text-xs text-on-surface-variant truncate mt-0.5">
                  Fast camera
                </span>
              </div>
            </button>
          </div>
        </div>

        {/* 2-Column Section: Network Bridges + Utilities */}
        <div className="grid grid-cols-1 lg:grid-cols-12 gap-8 mb-8">
          {/* Network Bridges (7 cols) */}
          <div className="lg:col-span-7 space-y-6">
            <span className="font-label-code text-xs font-bold text-on-surface-variant uppercase tracking-wider block">
              Network Bridges &amp; Agent Infrastructure
            </span>

            {/* Mobile Money Interoperability Card */}
            <div className="rounded-2xl bg-surface-container-lowest border border-surface-container-high/60 p-6 sm:p-7 shadow-sm space-y-5">
              <div className="flex items-start gap-4">
                <div className="w-14 h-14 rounded-2xl bg-surface-container flex items-center justify-center text-primary shrink-0">
                  <span className="material-symbols-outlined text-[28px]">sync_alt</span>
                </div>
                <div className="flex-1">
                  <div className="flex flex-wrap items-center gap-2">
                    <h2 className="font-headline-sm text-lg font-bold text-on-surface">
                      Transfer to Mobile Money
                    </h2>
                    <span className="px-2.5 py-0.5 rounded-full bg-tertiary-fixed text-on-tertiary-fixed font-label-code text-xs font-bold">
                      0% NETWORK FEE
                    </span>
                  </div>
                  <p className="font-body-sm text-sm text-on-surface-variant mt-1.5 leading-relaxed">
                    Direct real-time payout to Africell Money, Orange Money, and QMoney across Sierra Leone.
                  </p>
                  <div className="flex flex-wrap items-center gap-2 mt-4">
                    <span className="px-3 py-1 rounded-full bg-surface-container-high font-label-code text-xs font-medium text-on-surface">
                      Africell Money
                    </span>
                    <span className="px-3 py-1 rounded-full bg-surface-container-high font-label-code text-xs font-medium text-on-surface">
                      Orange Money
                    </span>
                    <span className="px-3 py-1 rounded-full bg-surface-container-high font-label-code text-xs font-medium text-on-surface">
                      QMoney SL
                    </span>
                  </div>
                </div>
              </div>
              <button
                className="w-full h-11 rounded-lg bg-primary text-on-primary font-semibold text-sm flex items-center justify-center gap-2 active:scale-[0.99] transition-transform shadow-xs cursor-pointer"
                type="button"
                onClick={() => onNavigate('send-step-1')}
              >
                <span>Initiate Telco Transfer</span>
                <span className="material-symbols-outlined text-[18px]">chevron_right</span>
              </button>
            </div>

            {/* Agent Cash In / Cash Out Hub */}
            <div className="rounded-2xl bg-surface-container-lowest border border-surface-container-high/60 p-6 shadow-sm">
              <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
                <div className="flex items-center gap-4">
                  <div className="w-12 h-12 rounded-xl bg-secondary-fixed flex items-center justify-center text-on-secondary-fixed shrink-0">
                    <span className="material-symbols-outlined text-[24px]">storefront</span>
                  </div>
                  <div>
                    <h2 className="font-headline-sm text-base font-bold text-on-surface">
                      Cash In / Cash Out Agents
                    </h2>
                    <p className="font-body-sm text-xs text-on-surface-variant mt-0.5">
                      1,400+ Verified cash agents across Freetown, Bo, Kenema, and Makeni
                    </p>
                  </div>
                </div>
                <button
                  className="h-10 px-5 rounded-lg bg-surface-container text-on-surface font-semibold text-xs hover:bg-surface-container-high transition-colors shrink-0 cursor-pointer"
                  type="button"
                  onClick={() => showToast('Locating 12 cash agents nearby in Freetown...', 'location_on')}
                >
                  Find Nearest Agent
                </button>
              </div>
            </div>
          </div>

          {/* Bill Payments & Utilities (5 cols) */}
          <div className="lg:col-span-5 space-y-6">
            <div className="flex items-center justify-between">
              <span className="font-label-code text-xs font-bold text-on-surface-variant uppercase tracking-wider">
                Utility &amp; Bill Payments
              </span>
              <button
                className="text-xs text-primary font-semibold flex items-center gap-0.5 hover:underline cursor-pointer"
                type="button"
                onClick={() => showToast('Opening all 24 registered utility billers...', 'receipt_long')}
              >
                <span>See all utilities</span>
                <span className="material-symbols-outlined text-[14px]">chevron_right</span>
              </button>
            </div>

            <div className="grid grid-cols-2 gap-4">
              <button
                className="flex flex-col items-center gap-3 p-5 rounded-xl bg-surface-container-lowest border border-surface-container-high/60 shadow-xs hover:border-primary/50 transition-all text-center cursor-pointer group"
                type="button"
                onClick={() => showToast('Selected Airtime Top-Up', 'phone_android')}
              >
                <div className="w-12 h-12 rounded-full bg-surface-container flex items-center justify-center text-primary group-hover:bg-primary group-hover:text-on-primary transition-all">
                  <span className="material-symbols-outlined text-[24px]">phone_android</span>
                </div>
                <div>
                  <span className="font-semibold text-sm block">Airtime</span>
                  <span className="text-xs text-on-surface-variant">Instant Reload</span>
                </div>
              </button>

              <button
                className="flex flex-col items-center gap-3 p-5 rounded-xl bg-surface-container-lowest border border-surface-container-high/60 shadow-xs hover:border-primary/50 transition-all text-center cursor-pointer group"
                type="button"
                onClick={() => showToast('Selected EDSA Power Token Purchase', 'bolt')}
              >
                <div className="w-12 h-12 rounded-full bg-surface-container flex items-center justify-center text-tertiary group-hover:bg-tertiary group-hover:text-on-tertiary transition-all">
                  <span className="material-symbols-outlined text-[24px]">bolt</span>
                </div>
                <div>
                  <span className="font-semibold text-sm block">EDSA Power</span>
                  <span className="text-xs text-on-surface-variant">Pre-paid Token</span>
                </div>
              </button>

              <button
                className="flex flex-col items-center gap-3 p-5 rounded-xl bg-surface-container-lowest border border-surface-container-high/60 shadow-xs hover:border-primary/50 transition-all text-center cursor-pointer group"
                type="button"
                onClick={() => showToast('Selected National Revenue Authority (NRA) Portal', 'account_balance')}
              >
                <div className="w-12 h-12 rounded-full bg-surface-container flex items-center justify-center text-secondary group-hover:bg-secondary group-hover:text-on-secondary transition-all">
                  <span className="material-symbols-outlined text-[24px]">account_balance</span>
                </div>
                <div>
                  <span className="font-semibold text-sm block">NRA / City</span>
                  <span className="text-xs text-on-surface-variant">Govt Taxes</span>
                </div>
              </button>

              <button
                className="flex flex-col items-center gap-3 p-5 rounded-xl bg-surface-container-lowest border border-surface-container-high/60 shadow-xs hover:border-primary/50 transition-all text-center cursor-pointer group"
                type="button"
                onClick={() => showToast('Selected Data Bundles (4G/5G LTE)', 'wifi_tethering')}
              >
                <div className="w-12 h-12 rounded-full bg-surface-container flex items-center justify-center text-primary group-hover:bg-primary group-hover:text-on-primary transition-all">
                  <span className="material-symbols-outlined text-[24px]">wifi_tethering</span>
                </div>
                <div>
                  <span className="font-semibold text-sm block">Data Bundles</span>
                  <span className="text-xs text-on-surface-variant">4G/5G LTE</span>
                </div>
              </button>
            </div>

            {/* Merchant Banner */}
            <div className="p-6 rounded-2xl bg-inverse-surface text-inverse-on-surface shadow-md space-y-4">
              <div className="flex items-center gap-2">
                <span className="material-symbols-outlined text-[20px] text-primary-fixed">point_of_sale</span>
                <span className="font-label-code text-xs text-primary-fixed uppercase tracking-wider font-semibold">
                  Anbarr Merchant Suite
                </span>
              </div>
              <div>
                <h3 className="font-headline-sm text-base font-bold text-inverse-on-surface">
                  Accelerate Counter Sales
                </h3>
                <p className="font-body-sm text-xs text-surface-variant/80 mt-1 leading-relaxed">
                  Generate instant verified POS receipts, automate local tax reconciliation, and access daily end-of-day settlement reports.
                </p>
              </div>
              <div className="flex items-center gap-4 pt-1">
                <button
                  className="h-10 px-5 rounded-lg bg-primary-container text-on-primary-container text-xs font-semibold hover:opacity-95 transition-opacity active:scale-[0.98] shadow-xs flex items-center gap-2 cursor-pointer"
                  type="button"
                  onClick={() => showToast('Merchant Portal active for Lumley branch', 'storefront')}
                >
                  <span>Open Merchant Portal</span>
                  <span className="material-symbols-outlined text-[15px]">launch</span>
                </button>
                <span className="text-surface-dim font-label-code text-[11px] flex items-center gap-1">
                  <span className="material-symbols-outlined text-[14px]">lock</span>
                  Tier-3 Verified
                </span>
              </div>
            </div>
          </div>
        </div>
      </main>

      <WebFooter onNavigate={onNavigate} showToast={showToast} />

      <div className="md:hidden">
        <BottomNav currentTab="services" onNavigate={onNavigate} />
      </div>
    </div>
  );
};

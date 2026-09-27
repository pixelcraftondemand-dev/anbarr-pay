import React, { useState } from 'react';
import { ScreenId } from '../../types';
import { WebNavbar, KADIATU_AVATAR } from '../WebNavbar';
import { WebFooter } from '../WebFooter';
import { BottomNav } from '../BottomNav';

interface WalletHomeScreenProps {
  onNavigate: (screen: ScreenId) => void;
  showToast: (msg: string, icon?: string) => void;
}

export const WalletHomeScreen: React.FC<WalletHomeScreenProps> = ({ onNavigate, showToast }) => {
  const [balanceVisible, setBalanceVisible] = useState(true);
  const [liquidityExpanded, setLiquidityExpanded] = useState(true);

  return (
    <div className="flex flex-col min-h-screen bg-surface font-body-md text-body-md text-on-surface antialiased selection:bg-primary selection:text-on-primary">
      {/* Responsive Web Top Navigation */}
      <WebNavbar
        currentScreen="wallet"
        onNavigate={onNavigate}
        onNotificationClick={() => showToast('3 unread central ledger alerts', 'notifications')}
        onSupportClick={() => showToast('Connecting to Freetown support desk...', 'support_agent')}
      />

      <main className="flex-1 w-full max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 pt-6 pb-20">
        {/* Web Breadcrumbs & Quick Header */}
        <div className="flex flex-col sm:flex-row sm:items-center justify-between pb-6 gap-4 border-b border-surface-container-high/60 mb-6">
          <div className="flex items-center gap-4">
            <div className="relative">
              <img
                className="w-14 h-14 rounded-full object-cover shadow-sm ring-2 ring-primary/30"
                alt="Kadiatu K."
                src={KADIATU_AVATAR}
                onError={(e) => {
                  e.currentTarget.src =
                    'https://images.unsplash.com/photo-1573496359142-b8d87734a5a2?w=150&auto=format&fit=crop&q=80';
                }}
              />
              <span className="absolute bottom-0 right-0 w-3.5 h-3.5 bg-primary rounded-full ring-2 ring-surface"></span>
            </div>
            <div className="flex flex-col">
              <div className="flex items-center gap-2">
                <span className="font-body-sm text-sm text-on-surface-variant">Welcome back,</span>
                <span className="px-2.5 py-0.5 rounded-full bg-primary/10 text-primary font-label-code text-[11px] font-bold tracking-wider">
                  TIER 3 AUDITED
                </span>
              </div>
              <h1 className="font-headline-md text-2xl font-bold text-on-surface tracking-tight">
                Kadiatu Kamara
              </h1>
            </div>
          </div>

          <div className="flex items-center gap-3">
            <button
              type="button"
              onClick={() => showToast('Generating invoice payment link...', 'link')}
              className="px-4 py-2 rounded-lg bg-surface-container-lowest hover:bg-surface-container text-on-surface text-sm font-semibold border border-surface-container-high shadow-xs transition-colors cursor-pointer flex items-center gap-2"
            >
              <span className="material-symbols-outlined text-[18px] rotate-180 text-primary">arrow_outward</span>
              <span>Request Payment</span>
            </button>
            <button
              type="button"
              onClick={() => onNavigate('send-step-1')}
              className="px-5 py-2 rounded-lg bg-primary hover:bg-primary-container text-on-primary text-sm font-semibold shadow-sm transition-all cursor-pointer flex items-center gap-2 active:scale-95"
            >
              <span className="material-symbols-outlined text-[18px]">send</span>
              <span>Send Money</span>
            </button>
          </div>
        </div>

        {/* 2-Column Responsive Dashboard Layout */}
        <div className="grid grid-cols-1 lg:grid-cols-12 gap-8">
          {/* Left Main Column (8 cols on desktop) */}
          <div className="lg:col-span-8 space-y-6">
            {/* Primary Balance Card (Ledger Vault) */}
            <div className="relative w-full rounded-2xl bg-gradient-to-br from-[#082032] via-[#0b2d42] to-primary overflow-hidden shadow-xl p-6 sm:p-8 text-on-primary">
              <svg
                className="absolute -right-10 -bottom-12 w-64 h-64 opacity-10 text-on-primary pointer-events-none"
                fill="currentColor"
                viewBox="0 0 100 100"
              >
                <circle cx="50" cy="50" fill="none" r="45" stroke="currentColor" strokeWidth="6"></circle>
                <path d="M50 15 L50 85 M15 50 L85 50" stroke="currentColor" strokeWidth="4"></path>
                <circle cx="50" cy="50" fill="none" r="22" stroke="currentColor" strokeWidth="4"></circle>
              </svg>
              <div className="absolute -top-16 -left-16 w-48 h-48 bg-primary-fixed/20 rounded-full blur-3xl pointer-events-none"></div>

              <div className="relative z-10 flex flex-col space-y-6">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2.5">
                    <div className="w-2.5 h-2.5 rounded-full bg-primary-fixed animate-pulse"></div>
                    <span className="font-label-code text-xs text-primary-fixed font-semibold uppercase tracking-wider">
                      Main Ledger Account · Primary Clearing
                    </span>
                  </div>
                  <div className="flex items-center gap-1.5 bg-surface-container-lowest/15 backdrop-blur-md px-3 py-1 rounded-full">
                    <span className="font-label-code text-xs text-on-primary font-medium tracking-wide">
                      ● SL-8849-01
                    </span>
                  </div>
                </div>

                <div className="flex flex-col py-1 space-y-2">
                  <div className="flex items-center gap-2">
                    <span className="text-sm text-surface-container-high/80">
                      Current available balance
                    </span>
                    <button
                      aria-label="Toggle Balance Visibility"
                      className="text-surface-container-high/80 hover:text-on-primary transition-colors flex items-center p-0.5 cursor-pointer"
                      id="toggle-balance-btn"
                      type="button"
                      onClick={() => setBalanceVisible(!balanceVisible)}
                    >
                      <span className="material-symbols-outlined text-[18px]" id="toggle-balance-icon">
                        {balanceVisible ? 'visibility' : 'visibility_off'}
                      </span>
                    </button>
                  </div>
                  <div className="flex items-baseline gap-3 pt-0.5">
                    <span className="font-headline-md text-2xl sm:text-3xl text-primary-fixed-dim font-bold">
                      SLE
                    </span>
                    {balanceVisible ? (
                      <span
                        className="font-ledger-number-lg text-3xl sm:text-4xl lg:text-5xl font-bold tracking-tight text-on-primary"
                        id="balance-amount"
                      >
                        48,250.00
                      </span>
                    ) : (
                      <span
                        className="font-ledger-number-lg text-3xl sm:text-4xl lg:text-5xl font-bold tracking-wider text-on-primary"
                        id="balance-hidden"
                      >
                        ••••••••
                      </span>
                    )}
                  </div>
                </div>

                <div className="flex flex-wrap items-center gap-3 pt-2">
                  <button
                    className="flex items-center justify-center gap-2 h-11 px-6 rounded-lg bg-primary hover:bg-primary-container text-on-primary text-sm font-semibold shadow-md active:scale-95 transition-all cursor-pointer"
                    type="button"
                    onClick={() => showToast('Opening instant cash-in channels...', 'account_balance')}
                  >
                    <span className="material-symbols-outlined text-[18px]">add_circle</span>
                    <span>Add Money</span>
                  </button>
                  <button
                    className="flex items-center justify-center gap-1.5 px-6 h-11 rounded-lg bg-surface-container-lowest/15 hover:bg-surface-container-lowest/25 backdrop-blur-md text-on-primary text-sm font-medium transition-all active:scale-95 cursor-pointer"
                    type="button"
                    onClick={() => onNavigate('services')}
                  >
                    <span>Account Details</span>
                    <span className="material-symbols-outlined text-[16px]">arrow_forward</span>
                  </button>
                  <button
                    className="flex items-center justify-center gap-1.5 px-4 h-11 rounded-lg bg-surface-container-lowest/10 hover:bg-surface-container-lowest/20 backdrop-blur-md text-on-primary text-sm font-medium transition-all cursor-pointer"
                    type="button"
                    onClick={() => onNavigate('cards')}
                  >
                    <span className="material-symbols-outlined text-[16px]">credit_card</span>
                    <span>View Virtual Card</span>
                  </button>
                </div>
              </div>
            </div>

            {/* Quick Action Matrix (Website Card Grid) */}
            <div className="rounded-xl bg-surface-container-lowest p-6 shadow-sm border border-surface-container-high/60">
              <h3 className="text-xs font-bold uppercase tracking-wider text-on-surface-variant mb-4">
                Fast Execution Hub
              </h3>
              <div className="grid grid-cols-2 sm:grid-cols-4 gap-4">
                {/* Send button - matches xpath //button[contains(., 'Send') and not(contains(., 'Add Money'))] */}
                <button
                  className="flex flex-col items-center gap-2.5 p-4 rounded-xl bg-surface-container-low/70 hover:bg-primary hover:text-on-primary transition-all duration-200 group active:scale-95 cursor-pointer text-left"
                  type="button"
                  onClick={() => onNavigate('send-step-1')}
                >
                  <div className="w-12 h-12 rounded-xl bg-surface-container-lowest shadow-sm flex items-center justify-center text-primary group-hover:bg-primary-container group-hover:text-on-primary transition-all">
                    <span className="material-symbols-outlined text-[24px]">arrow_outward</span>
                  </div>
                  <div className="text-center">
                    <span className="font-semibold text-sm block">Send</span>
                    <span className="text-[11px] text-on-surface-variant group-hover:text-on-primary/80 font-label-code">
                      Instant P2P
                    </span>
                  </div>
                </button>

                {/* Request */}
                <button
                  className="flex flex-col items-center gap-2.5 p-4 rounded-xl bg-surface-container-low/70 hover:bg-primary hover:text-on-primary transition-all duration-200 group active:scale-95 cursor-pointer text-left"
                  type="button"
                  onClick={() => showToast('Generating invoice payment link...', 'link')}
                >
                  <div className="w-12 h-12 rounded-xl bg-surface-container-lowest shadow-sm flex items-center justify-center text-primary group-hover:bg-primary-container group-hover:text-on-primary transition-all">
                    <span className="material-symbols-outlined text-[24px] rotate-180">arrow_outward</span>
                  </div>
                  <div className="text-center">
                    <span className="font-semibold text-sm block">Request</span>
                    <span className="text-[11px] text-on-surface-variant group-hover:text-on-primary/80 font-label-code">
                      Payment Link
                    </span>
                  </div>
                </button>

                {/* Pay / Scan */}
                <button
                  className="flex flex-col items-center gap-2.5 p-4 rounded-xl bg-surface-container-low/70 hover:bg-primary hover:text-on-primary transition-all duration-200 group active:scale-95 cursor-pointer text-left"
                  type="button"
                  onClick={() => showToast('Opening Merchant QR camera scanner...', 'qr_code_scanner')}
                >
                  <div className="w-12 h-12 rounded-xl bg-surface-container-lowest shadow-sm flex items-center justify-center text-primary group-hover:bg-primary-container group-hover:text-on-primary transition-all">
                    <span className="material-symbols-outlined text-[24px]">qr_code_scanner</span>
                  </div>
                  <div className="text-center">
                    <span className="font-semibold text-sm block">Pay</span>
                    <span className="text-[11px] text-on-surface-variant group-hover:text-on-primary/80 font-label-code">
                      QR Terminal
                    </span>
                  </div>
                </button>

                {/* More / Services */}
                <button
                  className="flex flex-col items-center gap-2.5 p-4 rounded-xl bg-surface-container-low/70 hover:bg-primary hover:text-on-primary transition-all duration-200 group active:scale-95 cursor-pointer text-left"
                  type="button"
                  onClick={() => onNavigate('services')}
                >
                  <div className="w-12 h-12 rounded-xl bg-surface-container-lowest shadow-sm flex items-center justify-center text-on-surface-variant group-hover:bg-primary-container group-hover:text-on-primary transition-all">
                    <span className="material-symbols-outlined text-[24px]">grid_view</span>
                  </div>
                  <div className="text-center">
                    <span className="font-semibold text-sm block">More</span>
                    <span className="text-[11px] text-on-surface-variant group-hover:text-on-primary/80 font-label-code">
                      All Services
                    </span>
                  </div>
                </button>
              </div>
            </div>

            {/* Recent Transactions Section */}
            <div className="rounded-xl bg-surface-container-lowest p-6 shadow-sm border border-surface-container-high/60 space-y-4">
              <div className="flex items-center justify-between pb-2 border-b border-surface-container-high/60">
                <div className="flex items-center gap-3">
                  <h3 className="font-headline-sm text-lg font-bold text-on-surface">Transactions Stream</h3>
                  <span className="px-2.5 py-0.5 rounded-full bg-surface-container-high text-on-surface-variant font-label-code text-xs">
                    4 Today
                  </span>
                </div>
                <button
                  className="text-xs font-semibold text-primary flex items-center gap-1 hover:underline cursor-pointer"
                  type="button"
                  onClick={() => onNavigate('transaction-details')}
                >
                  <span>View all history</span>
                  <span className="material-symbols-outlined text-[16px]">chevron_right</span>
                </button>
              </div>

              {/* Transaction Stream List */}
              <div className="space-y-2">
                {/* Item 1: Africell Money Inflow */}
                <div
                  className="flex items-center justify-between p-3.5 hover:bg-surface-container-low/60 rounded-xl transition-colors cursor-pointer border border-transparent hover:border-surface-container-high/60"
                  onClick={() => showToast('Africell Cash-in Settled #TX-9014', 'check_circle')}
                >
                  <div className="flex items-center gap-3.5 min-w-0">
                    <div className="w-11 h-11 rounded-xl bg-tertiary-fixed/30 flex items-center justify-center text-tertiary flex-shrink-0">
                      <span className="material-symbols-outlined text-[20px]">phone_android</span>
                    </div>
                    <div className="flex flex-col min-w-0">
                      <span className="font-semibold text-on-surface truncate">
                        Africell Money Cash-in
                      </span>
                      <span className="font-label-code text-xs text-on-surface-variant mt-0.5">
                        Sep 9, 19:39 · Reference #TX-9014
                      </span>
                    </div>
                  </div>
                  <div className="flex flex-col items-end flex-shrink-0 pl-3">
                    <span className="font-ledger-number-md font-bold text-tertiary">
                      +SLE 5,000.00
                    </span>
                    <span className="inline-flex items-center gap-1 font-label-code text-[11px] text-tertiary font-semibold mt-0.5">
                      <span className="w-1.5 h-1.5 rounded-full bg-tertiary"></span>Settled
                    </span>
                  </div>
                </div>

                {/* Item 2: Lumley Market SME Merchant */}
                {/* Exact match for xpath: //div[contains(., 'Lumley Market SME Merchant') and contains(@class, 'hover:bg-surface-container-low/60')] */}
                <div
                  className="flex items-center justify-between p-3.5 hover:bg-surface-container-low/60 rounded-xl transition-colors cursor-pointer border border-transparent hover:border-surface-container-high/60"
                  onClick={() => onNavigate('transaction-details')}
                >
                  <div className="flex items-center gap-3.5 min-w-0">
                    <div className="w-11 h-11 rounded-xl bg-surface-container-high flex items-center justify-center text-secondary flex-shrink-0">
                      <span className="material-symbols-outlined text-[20px]">storefront</span>
                    </div>
                    <div className="flex flex-col min-w-0">
                      <span className="font-semibold text-on-surface truncate">
                        Lumley Market SME Merchant
                      </span>
                      <span className="font-label-code text-xs text-on-surface-variant mt-0.5">
                        Sep 9, 14:15 · QR Terminal Freetown
                      </span>
                    </div>
                  </div>
                  <div className="flex flex-col items-end flex-shrink-0 pl-3">
                    <span className="font-ledger-number-md font-bold text-on-surface">
                      -SLE 340.00
                    </span>
                    <span className="font-label-code text-[11px] text-on-surface-variant mt-0.5">
                      POS Direct
                    </span>
                  </div>
                </div>

                {/* Item 3: EDSA Electricity Bill */}
                <div
                  className="flex items-center justify-between p-3.5 hover:bg-surface-container-low/60 rounded-xl transition-colors cursor-pointer border border-transparent hover:border-surface-container-high/60"
                  onClick={() => showToast('EDSA Token #821: 4920-1849-0192', 'bolt')}
                >
                  <div className="flex items-center gap-3.5 min-w-0">
                    <div className="w-11 h-11 rounded-xl bg-secondary-fixed/50 flex items-center justify-center text-on-secondary-fixed flex-shrink-0">
                      <span className="material-symbols-outlined text-[20px]">bolt</span>
                    </div>
                    <div className="flex flex-col min-w-0">
                      <span className="font-semibold text-on-surface truncate">
                        EDSA Pre-paid Electricity
                      </span>
                      <span className="font-label-code text-xs text-on-surface-variant mt-0.5">
                        Sep 8, 11:20 · Token #821
                      </span>
                    </div>
                  </div>
                  <div className="flex flex-col items-end flex-shrink-0 pl-3">
                    <span className="font-ledger-number-md font-bold text-on-surface">
                      -SLE 150.00
                    </span>
                    <span className="font-label-code text-[11px] text-on-surface-variant mt-0.5">
                      Utility Token
                    </span>
                  </div>
                </div>

                {/* Item 4: Regulatory Excise & Audit Fee */}
                <div
                  className="flex items-center justify-between p-3.5 hover:bg-surface-container-low/60 rounded-xl transition-colors cursor-pointer border border-transparent hover:border-surface-container-high/60"
                  onClick={() => showToast('BSL Statutory Excise: 0.05% of turnover', 'receipt_long')}
                >
                  <div className="flex items-center gap-3.5 min-w-0">
                    <div className="w-11 h-11 rounded-xl bg-surface-container flex items-center justify-center text-outline flex-shrink-0">
                      <span className="material-symbols-outlined text-[20px]">receipt_long</span>
                    </div>
                    <div className="flex flex-col min-w-0">
                      <span className="font-semibold text-on-surface truncate">
                        Transaction Fee &amp; Excise
                      </span>
                      <span className="font-label-code text-xs text-on-surface-variant mt-0.5">
                        Sep 8, 11:20 · BSL Statutory
                      </span>
                    </div>
                  </div>
                  <div className="flex flex-col items-end flex-shrink-0 pl-3">
                    <span className="font-ledger-number-md font-semibold text-on-surface-variant">
                      -SLE 0.58
                    </span>
                    <span className="font-label-code text-[11px] text-on-surface-variant mt-0.5">
                      Automated
                    </span>
                  </div>
                </div>
              </div>
            </div>
          </div>

          {/* Right Sidebar Column (4 cols on desktop) */}
          <div className="lg:col-span-4 space-y-6">
            {/* Insights Panel */}
            <div className="rounded-xl bg-surface-container-lowest p-6 shadow-sm border border-surface-container-high/60 space-y-4">
              <div className="flex items-center justify-between">
                <h3 className="font-headline-sm text-base font-bold text-on-surface">
                  Monthly Financial Insights
                </h3>
                <button
                  className="text-xs text-primary font-semibold flex items-center gap-0.5 hover:underline cursor-pointer"
                  type="button"
                  onClick={() => onNavigate('cards')}
                >
                  <span>Analytics</span>
                  <span className="material-symbols-outlined text-[14px]">chevron_right</span>
                </button>
              </div>

              <div className="grid grid-cols-2 gap-3">
                <div className="p-4 rounded-xl bg-surface-container-low/70 flex flex-col justify-between space-y-2">
                  <div className="flex items-center justify-between">
                    <div className="w-8 h-8 rounded-full bg-tertiary-fixed/40 flex items-center justify-center text-tertiary">
                      <span className="material-symbols-outlined text-[16px]">south_west</span>
                    </div>
                    <span className="font-label-code text-xs text-tertiary font-bold">+18.4%</span>
                  </div>
                  <div className="flex flex-col">
                    <span className="text-xs text-on-surface-variant">Income</span>
                    <span className="font-ledger-number-md font-bold text-tertiary truncate">
                      +SLE 24,800
                    </span>
                  </div>
                </div>

                <div className="p-4 rounded-xl bg-surface-container-low/70 flex flex-col justify-between space-y-2">
                  <div className="flex items-center justify-between">
                    <div className="w-8 h-8 rounded-full bg-error-container/60 flex items-center justify-center text-error">
                      <span className="material-symbols-outlined text-[16px]">north_east</span>
                    </div>
                    <span className="font-label-code text-xs text-error font-bold">-4.2%</span>
                  </div>
                  <div className="flex flex-col">
                    <span className="text-xs text-on-surface-variant">Spending</span>
                    <span className="font-ledger-number-md font-bold text-on-surface truncate">
                      -SLE 6,450
                    </span>
                  </div>
                </div>
              </div>
            </div>

            {/* Quick Financial Settlement Channels */}
            <div className="rounded-xl bg-surface-container-lowest p-6 shadow-sm border border-surface-container-high/60 space-y-4">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <span className="material-symbols-outlined text-primary text-[20px]">sync_alt</span>
                  <span className="font-label-code text-xs font-bold uppercase tracking-wider text-on-surface">
                    Liquidity Channels
                  </span>
                </div>
                <button
                  aria-label="Expand liquidity channels"
                  className="w-7 h-7 rounded-lg bg-surface-container-high flex items-center justify-center text-on-surface-variant hover:text-on-surface transition-colors cursor-pointer"
                  type="button"
                  onClick={() => setLiquidityExpanded(!liquidityExpanded)}
                >
                  <span
                    className="material-symbols-outlined text-[18px] transition-transform duration-200"
                    style={{ transform: liquidityExpanded ? 'rotate(0deg)' : 'rotate(180deg)' }}
                  >
                    expand_more
                  </span>
                </button>
              </div>

              {liquidityExpanded && (
                <div className="grid grid-cols-3 gap-2.5">
                  <div className="flex flex-col bg-surface-container-low/70 p-3 rounded-xl">
                    <span className="text-[11px] text-on-surface-variant font-medium">Add Money</span>
                    <span className="font-ledger-number-md text-xs font-bold text-on-surface mt-1 truncate">
                      SLE 12.5k
                    </span>
                  </div>
                  <div className="flex flex-col bg-surface-container-low/70 p-3 rounded-xl">
                    <span className="text-[11px] text-on-surface-variant font-medium">Converter</span>
                    <span className="font-ledger-number-md text-xs font-bold text-secondary mt-1 truncate">
                      SLE 1,820
                    </span>
                  </div>
                  <div className="flex flex-col bg-surface-container-low/70 p-3 rounded-xl">
                    <span className="text-[11px] text-on-surface-variant font-medium">Load Card</span>
                    <span className="font-ledger-number-md text-xs font-bold text-primary mt-1 truncate">
                      SLE 8,000
                    </span>
                  </div>
                </div>
              )}
            </div>

            {/* Virtual Card Preview Widget on Website */}
            <div className="rounded-xl bg-gradient-to-br from-primary via-tertiary to-inverse-surface text-on-primary p-6 shadow-md relative overflow-hidden">
              <div className="flex items-center justify-between mb-4">
                <span className="text-xs uppercase font-label-code text-tertiary-fixed font-bold tracking-wider">
                  Virtual SLE Card
                </span>
                <span className="text-xs font-label-code text-white/80">VISA · Contactless</span>
              </div>
              <div className="space-y-2">
                <div className="text-xs text-white/70">Card Balance</div>
                <div className="font-ledger-number-lg text-2xl font-bold text-white">SLE 8,420.50</div>
                <div className="font-ledger-number-md text-sm tracking-widest text-white/90 pt-2">
                  •••• •••• •••• 3192
                </div>
              </div>
              <div className="mt-4 pt-4 border-t border-white/10 flex items-center justify-between">
                <span className="text-xs font-label-code text-white/70">Exp: 08/28</span>
                <button
                  type="button"
                  onClick={() => onNavigate('cards')}
                  className="text-xs font-semibold text-primary-fixed hover:text-white flex items-center gap-1 cursor-pointer"
                >
                  <span>Manage Card</span>
                  <span className="material-symbols-outlined text-[14px]">arrow_forward</span>
                </button>
              </div>
            </div>

            {/* Regulatory Seal */}
            <div className="rounded-xl bg-surface-container-lowest p-5 border border-surface-container-high/60 text-center space-y-2">
              <div className="flex items-center justify-center gap-1.5 text-primary">
                <span className="material-symbols-outlined text-[18px]">lock</span>
                <span className="font-label-code text-xs font-bold tracking-wider uppercase">
                  Bank of Sierra Leone Regulated
                </span>
              </div>
              <p className="text-xs text-on-surface-variant leading-relaxed">
                Central ledger clearance ensures all funds are cryptographically accounted for with instantaneous settlement.
              </p>
            </div>
          </div>
        </div>
      </main>

      {/* Website Footer */}
      <WebFooter onNavigate={onNavigate} showToast={showToast} />

      {/* Responsive Bottom Navigation for Mobile / Tablet Viewports */}
      <div className="md:hidden">
        <BottomNav currentTab="wallet" onNavigate={onNavigate} />
      </div>
    </div>
  );
};

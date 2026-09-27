import React, { useState } from 'react';
import { ScreenId } from '../../types';
import { WebNavbar } from '../WebNavbar';
import { WebFooter } from '../WebFooter';
import { BottomNav } from '../BottomNav';

interface CardsScreenProps {
  onNavigate: (screen: ScreenId) => void;
  showToast: (msg: string, icon?: string) => void;
}

export const CardsScreen: React.FC<CardsScreenProps> = ({ onNavigate, showToast }) => {
  const [balanceHidden, setBalanceHidden] = useState(false);
  const [cardFrozen, setCardFrozen] = useState(false);
  const [showCvv, setShowCvv] = useState(false);
  const [catCollapsed, setCatCollapsed] = useState(false);
  const [copyToast, setCopyToast] = useState(false);

  const handleCopyCard = () => {
    if (navigator.clipboard) {
      navigator.clipboard.writeText('4520 8920 1842 3192');
    }
    setCopyToast(true);
    setTimeout(() => setCopyToast(false), 2000);
  };

  const handleCvvClick = () => {
    setShowCvv(true);
    setTimeout(() => setShowCvv(false), 4000);
  };

  return (
    <div className="flex flex-col min-h-screen bg-surface font-body-md text-body-md text-on-surface antialiased selection:bg-primary selection:text-on-primary">
      <WebNavbar
        currentScreen="cards"
        onNavigate={onNavigate}
        onNotificationClick={() => showToast('Virtual card limit active', 'credit_card')}
        onSupportClick={() => showToast('Card dispute center online', 'support_agent')}
      />

      <main className="flex-1 w-full max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 pt-8 pb-20">
        {/* Top Header Row */}
        <div className="flex flex-col sm:flex-row sm:items-center justify-between pb-6 gap-4 border-b border-surface-container-high/60 mb-8">
          <div>
            <div className="flex items-center gap-3">
              <h1 className="font-headline-md text-2xl sm:text-3xl font-bold text-on-surface tracking-tight">
                Card Management &amp; Spend Controls
              </h1>
              <span className="px-2.5 py-0.5 rounded-full bg-primary/10 text-primary font-label-code text-xs font-bold tracking-wider uppercase">
                2 Active Cards
              </span>
            </div>
            <p className="font-body-sm text-sm text-on-surface-variant mt-1">
              Virtual SLE Debit &amp; Physical SME Merchant Cards with Real-Time Clearing
            </p>
          </div>

          <div className="flex items-center gap-3">
            <button
              className="flex items-center space-x-1.5 px-4 py-2 rounded-lg bg-surface-container-lowest hover:bg-surface-container text-on-surface border border-surface-container-high text-sm font-semibold transition-colors cursor-pointer"
              type="button"
              onClick={() => showToast('Card limits & security settings online', 'tune')}
            >
              <span className="material-symbols-outlined text-[18px]">tune</span>
              <span>Card Limits</span>
            </button>
            <button
              className="flex items-center space-x-1.5 px-4 py-2 rounded-lg bg-primary hover:bg-primary-container text-on-primary text-sm font-semibold shadow-xs transition-colors cursor-pointer active:scale-95"
              type="button"
              onClick={() => showToast('Issuing new contactless card...', 'add_card')}
            >
              <span className="material-symbols-outlined text-[18px]">add_circle</span>
              <span>Request New Card</span>
            </button>
          </div>
        </div>

        {/* 2-Column Responsive Layout */}
        <div className="grid grid-cols-1 lg:grid-cols-12 gap-8">
          {/* Left Column: Visual Card & Security Controls (5 cols on desktop) */}
          <div className="lg:col-span-5 space-y-6">
            {/* Virtual Debit Card Hero Card */}
            <div className="relative w-full rounded-2xl bg-gradient-to-br from-primary via-tertiary to-inverse-surface text-on-primary p-7 sm:p-8 shadow-xl overflow-hidden select-none">
              <div className="absolute -right-16 -top-16 w-52 h-52 rounded-full bg-primary-fixed/20 blur-3xl pointer-events-none"></div>
              <div className="absolute -left-12 -bottom-12 w-48 h-48 rounded-full bg-secondary-container/15 blur-2xl pointer-events-none"></div>
              <div className="absolute inset-0 bg-gradient-to-tr from-transparent via-surface-container-lowest/5 to-transparent pointer-events-none"></div>

              <div className="relative z-10 flex items-center justify-between">
                <div className="flex items-center space-x-2.5">
                  <span className="font-headline-sm text-lg tracking-tight font-bold text-white">ANBARR</span>
                  <span className="w-1.5 h-1.5 rounded-full bg-primary-fixed"></span>
                  <span className="font-label-code text-[11px] tracking-widest uppercase text-tertiary-fixed font-semibold">
                    Virtual
                  </span>
                </div>
                <div className="flex items-center space-x-2.5">
                  <span className="material-symbols-outlined text-[20px] text-white/80 rotate-90">contactless</span>
                  <span className="px-2.5 py-1 rounded-md bg-white/10 backdrop-blur-sm text-white font-label-code text-[10px] font-medium tracking-wide">
                    SLE DEBIT
                  </span>
                </div>
              </div>

              <div className="relative z-10 mt-8 flex items-center justify-between">
                <div className="w-12 h-9 rounded-lg bg-gradient-to-br from-amber-200 via-yellow-400 to-amber-600 shadow-sm flex flex-col justify-between p-1.5 relative overflow-hidden">
                  <div className="w-full h-[1px] bg-amber-800/40"></div>
                  <div className="flex justify-between items-center h-full">
                    <div className="w-2.5 h-full rounded-sm border border-amber-900/30"></div>
                    <div className="w-3.5 h-full rounded-sm border border-amber-900/30"></div>
                  </div>
                  <div className="w-full h-[1px] bg-amber-800/40"></div>
                </div>

                <div className="flex flex-col items-end">
                  <span className="font-label-code text-[10px] uppercase text-white/60 tracking-wider">Card Balance</span>
                  <div className="flex items-center space-x-2 mt-1">
                    <span className="font-ledger-number-md text-lg text-white font-bold tracking-tight" id="card-balance-val">
                      {balanceHidden ? 'SLE ••••••••' : 'SLE 8,420.50'}
                    </span>
                    <button
                      className="w-6 h-6 flex items-center justify-center rounded-full hover:bg-white/10 text-white/80 transition-colors cursor-pointer"
                      id="balance-toggle-btn"
                      title="Toggle balance view"
                      type="button"
                      onClick={() => setBalanceHidden(!balanceHidden)}
                    >
                      <span className="material-symbols-outlined text-[16px]" id="balance-eye-icon">
                        {balanceHidden ? 'visibility_off' : 'visibility'}
                      </span>
                    </button>
                  </div>
                </div>
              </div>

              <div className="relative z-10 mt-8 flex items-center justify-between">
                <div className="flex items-center space-x-3">
                  <span className="font-ledger-number-md text-[18px] sm:text-[20px] tracking-[0.18em] text-white font-semibold">
                    •••• •••• •••• 3192
                  </span>
                  <button
                    className="text-white/60 hover:text-white transition-colors p-1.5 rounded hover:bg-white/10 cursor-pointer"
                    id="copy-card-btn"
                    title="Copy Card Number"
                    type="button"
                    onClick={handleCopyCard}
                  >
                    <span className="material-symbols-outlined text-[17px]">content_copy</span>
                  </button>
                </div>
                {copyToast && (
                  <span className="font-label-code text-[11px] text-tertiary-fixed bg-white/10 px-2.5 py-0.5 rounded transition-all" id="copy-toast">
                    Copied!
                  </span>
                )}
              </div>

              <div className="relative z-10 mt-8 flex items-end justify-between pt-1">
                <div>
                  <span className="block font-label-code text-[9px] uppercase tracking-widest text-white/60 mb-1">
                    Cardholder Name
                  </span>
                  <span className="font-body-md text-sm font-semibold tracking-wider text-white uppercase">
                    Kadiatu Kamara
                  </span>
                </div>
                <div className="flex items-center space-x-4">
                  <div>
                    <span className="block font-label-code text-[9px] uppercase tracking-widest text-white/60 text-right mb-1">
                      Expires
                    </span>
                    <span className="font-label-code text-sm font-semibold text-white tracking-wider">08/28</span>
                  </div>
                  <div className="flex items-center italic font-headline-md text-xl tracking-tight font-extrabold text-white pl-1 select-none">
                    VISA
                  </div>
                </div>
              </div>
            </div>

            {/* Quick Card Action Buttons */}
            <div className="grid grid-cols-3 gap-3">
              <button
                className="flex flex-col items-center justify-center p-4 rounded-xl bg-surface-container-lowest border border-surface-container-high/60 shadow-xs hover:border-primary/50 transition-all active:scale-[0.98] group cursor-pointer"
                type="button"
                onClick={() => showToast('Loading SLE onto virtual card...', 'add_card')}
              >
                <div className="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center text-primary group-hover:bg-primary group-hover:text-on-primary transition-colors mb-2">
                  <span className="material-symbols-outlined text-[20px]">add_card</span>
                </div>
                <span className="font-semibold text-xs text-on-surface">Load Card</span>
                <span className="font-label-code text-[10px] text-on-surface-variant mt-0.5">Instant SLE</span>
              </button>

              <button
                className="flex flex-col items-center justify-center p-4 rounded-xl bg-surface-container-lowest border border-surface-container-high/60 shadow-xs hover:border-primary/50 transition-all active:scale-[0.98] group cursor-pointer"
                type="button"
                onClick={() => onNavigate('wallet')}
              >
                <div className="w-10 h-10 rounded-full bg-secondary/10 flex items-center justify-center text-secondary group-hover:bg-secondary group-hover:text-on-secondary transition-colors mb-2">
                  <span className="material-symbols-outlined text-[20px]">swap_horiz</span>
                </div>
                <span className="font-semibold text-xs text-on-surface">Transfer</span>
                <span className="font-label-code text-[10px] text-on-surface-variant mt-0.5">To Wallet</span>
              </button>

              <button
                className="flex flex-col items-center justify-center p-4 rounded-xl bg-surface-container-lowest border border-surface-container-high/60 shadow-xs hover:border-primary/50 transition-all active:scale-[0.98] group cursor-pointer"
                type="button"
                onClick={() => showToast('Card spending limits & PIN security', 'tune')}
              >
                <div className="w-10 h-10 rounded-full bg-surface-container-high flex items-center justify-center text-on-surface-variant group-hover:bg-inverse-surface group-hover:text-inverse-on-surface transition-colors mb-2">
                  <span className="material-symbols-outlined text-[20px]">tune</span>
                </div>
                <span className="font-semibold text-xs text-on-surface">Settings</span>
                <span className="font-label-code text-[10px] text-on-surface-variant mt-0.5">Limits &amp; PIN</span>
              </button>
            </div>

            {/* Security & Freeze Card Controls */}
            <div className="p-5 rounded-2xl bg-surface-container-lowest border border-surface-container-high/60 shadow-xs flex items-center justify-between">
              <div className="flex items-center space-x-3.5">
                <div className="w-10 h-10 rounded-xl bg-surface-container-low flex items-center justify-center text-on-surface">
                  <span className="material-symbols-outlined text-[20px]">ac_unit</span>
                </div>
                <div className="flex flex-col">
                  <span className="text-sm font-bold text-on-surface">Freeze Card</span>
                  <span
                    className={`font-label-code text-xs mt-0.5 ${
                      cardFrozen ? 'text-error font-semibold' : 'text-on-surface-variant'
                    }`}
                    id="freeze-status"
                  >
                    {cardFrozen ? 'Card Locked' : 'Temporarily disable online purchases'}
                  </span>
                </div>
              </div>

              <div className="flex items-center space-x-4">
                <label className="relative inline-flex items-center cursor-pointer">
                  <input
                    checked={cardFrozen}
                    className="sr-only peer"
                    id="freeze-toggle"
                    type="checkbox"
                    onChange={(e) => {
                      setCardFrozen(e.target.checked);
                      showToast(e.target.checked ? 'Card frozen instantly' : 'Card unfrozen', 'ac_unit');
                    }}
                  />
                  <div className="w-11 h-6 bg-surface-container-high peer-focus:outline-none rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-surface-variant after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-primary"></div>
                </label>

                <div className="h-6 w-px bg-surface-container-high"></div>

                <button
                  className="flex items-center space-x-1.5 py-1.5 px-3 rounded-lg bg-surface-container-low hover:bg-surface-container transition-colors cursor-pointer"
                  id="cvv-trigger-btn"
                  type="button"
                  onClick={handleCvvClick}
                >
                  <span className="material-symbols-outlined text-[16px] text-primary">fingerprint</span>
                  <span className="font-label-code text-xs font-bold text-on-surface tracking-wider" id="cvv-label">
                    {showCvv ? 'CVV: 742' : 'SHOW CVV'}
                  </span>
                </button>
              </div>
            </div>
          </div>

          {/* Right Column: Spending Analytics & History (7 cols on desktop) */}
          <div className="lg:col-span-7 space-y-6">
            {/* Card Insights & Spending Metric Row */}
            <div className="p-6 rounded-2xl bg-surface-container-lowest border border-surface-container-high/60 shadow-xs space-y-4">
              <div className="flex items-center justify-between pb-2 border-b border-surface-container-high/60">
                <div className="flex items-center space-x-2">
                  <span className="material-symbols-outlined text-[20px] text-primary">insights</span>
                  <span className="font-headline-sm text-base font-bold text-on-surface">Card Insights &amp; Utilization</span>
                </div>
                <button
                  className="flex items-center text-secondary hover:text-secondary-container transition-colors text-xs font-semibold cursor-pointer"
                  type="button"
                  onClick={() => showToast('Opening monthly expense analytics...', 'insights')}
                >
                  <span>Monthly view</span>
                  <span className="material-symbols-outlined text-[14px] ml-0.5">chevron_right</span>
                </button>
              </div>

              <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                <div className="p-4 rounded-xl bg-surface-container-low/70 flex flex-col justify-between space-y-3">
                  <span className="font-label-code text-xs uppercase tracking-wide text-on-surface-variant">
                    Card Spent This Month
                  </span>
                  <div className="flex items-baseline space-x-1">
                    <span className="font-ledger-number-md text-xl font-bold text-primary">
                      +SLE 3,120.00
                    </span>
                  </div>
                  <div className="flex items-center space-x-1 text-primary pt-1">
                    <span className="material-symbols-outlined text-[14px]">trending_up</span>
                    <span className="font-label-code text-xs font-semibold">20.8% of monthly limit</span>
                  </div>
                </div>

                <div className="p-4 rounded-xl bg-surface-container-low/70 flex flex-col justify-between space-y-3">
                  <span className="font-label-code text-xs uppercase tracking-wide text-on-surface-variant">
                    Authorized Card Limit
                  </span>
                  <div className="flex items-baseline space-x-1">
                    <span className="font-ledger-number-md text-xl font-bold text-on-surface">
                      SLE 15,000.00
                    </span>
                  </div>
                  <div className="w-full bg-surface-container-highest rounded-full h-1.5 overflow-hidden pt-0.5">
                    <div className="bg-primary h-1.5 rounded-full" style={{ width: '20.8%' }}></div>
                  </div>
                </div>
              </div>
            </div>

            {/* Quick Categories Summary Card */}
            <div className="p-6 rounded-2xl bg-surface-container-lowest border border-surface-container-high/60 shadow-xs space-y-4">
              <div className="flex items-center justify-between pb-2 border-b border-surface-container-high/60">
                <div className="flex items-center space-x-2">
                  <span className="material-symbols-outlined text-[20px] text-tertiary">donut_small</span>
                  <span className="font-headline-sm text-base font-bold text-on-surface">
                    Top Expense Categories
                  </span>
                </div>
                <button
                  className="w-7 h-7 flex items-center justify-center rounded-lg hover:bg-surface-container-high text-on-surface-variant transition-colors cursor-pointer"
                  id="toggle-cat-btn"
                  type="button"
                  onClick={() => setCatCollapsed(!catCollapsed)}
                >
                  <span className="material-symbols-outlined text-[18px]" id="cat-expand-icon">
                    {catCollapsed ? 'expand_more' : 'expand_less'}
                  </span>
                </button>
              </div>

              {!catCollapsed && (
                <div className="space-y-4 pt-1" id="categories-container">
                  <div className="flex flex-col space-y-1.5">
                    <div className="flex items-center justify-between text-xs">
                      <div className="flex items-center space-x-2">
                        <span className="w-2.5 h-2.5 rounded-full bg-primary"></span>
                        <span className="font-semibold text-on-surface">Rent &amp; Office Supplies</span>
                      </div>
                      <span className="font-ledger-number-md font-semibold text-on-surface">
                        SLE 1,200.00 (38.5%)
                      </span>
                    </div>
                    <div className="w-full bg-surface-container-low rounded-full h-2 overflow-hidden">
                      <div className="bg-primary h-2 rounded-full" style={{ width: '38.5%' }}></div>
                    </div>
                  </div>

                  <div className="flex flex-col space-y-1.5">
                    <div className="flex items-center justify-between text-xs">
                      <div className="flex items-center space-x-2">
                        <span className="w-2.5 h-2.5 rounded-full bg-secondary"></span>
                        <span className="font-semibold text-on-surface">Food &amp; Dining</span>
                      </div>
                      <span className="font-ledger-number-md font-semibold text-on-surface">
                        SLE 680.00 (21.8%)
                      </span>
                    </div>
                    <div className="w-full bg-surface-container-low rounded-full h-2 overflow-hidden">
                      <div className="bg-secondary h-2 rounded-full" style={{ width: '21.8%' }}></div>
                    </div>
                  </div>

                  <div className="flex flex-col space-y-1.5">
                    <div className="flex items-center justify-between text-xs">
                      <div className="flex items-center space-x-2">
                        <span className="w-2.5 h-2.5 rounded-full bg-tertiary-container"></span>
                        <span className="font-semibold text-on-surface">Transport &amp; Fuel</span>
                      </div>
                      <span className="font-ledger-number-md font-semibold text-on-surface">
                        SLE 450.00 (14.4%)
                      </span>
                    </div>
                    <div className="w-full bg-surface-container-low rounded-full h-2 overflow-hidden">
                      <div className="bg-tertiary-container h-2 rounded-full" style={{ width: '14.4%' }}></div>
                    </div>
                  </div>
                </div>
              )}
            </div>

            {/* Recent Card Transactions List */}
            <div className="p-6 rounded-2xl bg-surface-container-lowest border border-surface-container-high/60 shadow-xs space-y-4">
              <div className="flex items-center justify-between pb-2 border-b border-surface-container-high/60">
                <div className="flex items-center space-x-2">
                  <span className="material-symbols-outlined text-[20px] text-primary">receipt_long</span>
                  <span className="font-headline-sm text-base font-bold text-on-surface">
                    Recent Card Transactions
                  </span>
                </div>
                <button
                  className="flex items-center text-secondary hover:text-secondary-container transition-colors text-xs font-semibold cursor-pointer"
                  type="button"
                  onClick={() => onNavigate('transaction-details')}
                >
                  <span>View all</span>
                  <span className="material-symbols-outlined text-[14px] ml-0.5">chevron_right</span>
                </button>
              </div>

              <div className="space-y-2">
                <div className="flex items-center justify-between py-3 rounded-xl hover:bg-surface-container-low/50 px-3 transition-colors cursor-pointer border border-transparent hover:border-surface-container-high/60">
                  <div className="flex items-center space-x-3.5">
                    <div className="w-11 h-11 rounded-xl bg-surface-container-high flex items-center justify-center text-on-surface">
                      <span className="material-symbols-outlined text-[20px]">wifi</span>
                    </div>
                    <div className="flex flex-col">
                      <span className="font-semibold text-sm text-on-surface">
                        Starlink Internet Freetown
                      </span>
                      <div className="flex items-center space-x-2 mt-0.5">
                        <span className="font-label-code text-xs text-on-surface-variant">Sep 07, 10:14</span>
                        <span className="w-1 h-1 rounded-full bg-outline-variant"></span>
                        <span className="font-label-code text-xs text-primary font-medium">Web Merchant</span>
                      </div>
                    </div>
                  </div>
                  <div className="flex flex-col items-end">
                    <span className="font-ledger-number-md font-bold text-on-surface">
                      -SLE 890.00
                    </span>
                    <span className="font-label-code text-[11px] text-primary font-medium mt-0.5">Settled</span>
                  </div>
                </div>

                <div className="flex items-center justify-between py-3 rounded-xl hover:bg-surface-container-low/50 px-3 transition-colors cursor-pointer border border-transparent hover:border-surface-container-high/60">
                  <div className="flex items-center space-x-3.5">
                    <div className="w-11 h-11 rounded-xl bg-surface-container-high flex items-center justify-center text-on-surface">
                      <span className="material-symbols-outlined text-[20px]">local_gas_station</span>
                    </div>
                    <div className="flex flex-col">
                      <span className="font-semibold text-sm text-on-surface">
                        TotalEnergies Congo Cross
                      </span>
                      <div className="flex items-center space-x-2 mt-0.5">
                        <span className="font-label-code text-xs text-on-surface-variant">Sep 06, 17:30</span>
                        <span className="w-1 h-1 rounded-full bg-outline-variant"></span>
                        <span className="font-label-code text-xs text-on-surface-variant">POS Terminal</span>
                      </div>
                    </div>
                  </div>
                  <div className="flex flex-col items-end">
                    <span className="font-ledger-number-md font-bold text-on-surface">
                      -SLE 350.00
                    </span>
                    <span className="font-label-code text-[11px] text-primary font-medium mt-0.5">Settled</span>
                  </div>
                </div>

                <div className="flex items-center justify-between py-3 rounded-xl hover:bg-surface-container-low/50 px-3 transition-colors cursor-pointer border border-transparent hover:border-surface-container-high/60">
                  <div className="flex items-center space-x-3.5">
                    <div className="w-11 h-11 rounded-xl bg-surface-container-high flex items-center justify-center text-on-surface">
                      <span className="material-symbols-outlined text-[20px]">shopping_cart</span>
                    </div>
                    <div className="flex flex-col">
                      <span className="font-semibold text-sm text-on-surface">Supermarket Lumley</span>
                      <div className="flex items-center space-x-2 mt-0.5">
                        <span className="font-label-code text-xs text-on-surface-variant">Sep 05, 12:45</span>
                        <span className="w-1 h-1 rounded-full bg-outline-variant"></span>
                        <span className="font-label-code text-xs text-on-surface-variant">Contactless</span>
                      </div>
                    </div>
                  </div>
                  <div className="flex flex-col items-end">
                    <span className="font-ledger-number-md font-bold text-on-surface">
                      -SLE 520.00
                    </span>
                    <span className="font-label-code text-[11px] text-primary font-medium mt-0.5">Settled</span>
                  </div>
                </div>

                <div className="flex items-center justify-between py-3 rounded-xl hover:bg-surface-container-low/50 px-3 transition-colors cursor-pointer border border-transparent hover:border-surface-container-high/60">
                  <div className="flex items-center space-x-3.5">
                    <div className="w-11 h-11 rounded-xl bg-primary/10 flex items-center justify-center text-primary">
                      <span className="material-symbols-outlined text-[20px]">account_balance_wallet</span>
                    </div>
                    <div className="flex flex-col">
                      <span className="font-semibold text-sm text-on-surface">
                        Card Top-up from Wallet
                      </span>
                      <div className="flex items-center space-x-2 mt-0.5">
                        <span className="font-label-code text-xs text-on-surface-variant">Sep 05, 09:00</span>
                        <span className="w-1 h-1 rounded-full bg-outline-variant"></span>
                        <span className="font-label-code text-xs text-primary font-medium">Instant Load</span>
                      </div>
                    </div>
                  </div>
                  <div className="flex flex-col items-end">
                    <span className="font-ledger-number-md font-bold text-primary">
                      +SLE 2,000.00
                    </span>
                    <span className="font-label-code text-[11px] text-primary font-medium mt-0.5">Completed</span>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </main>

      <WebFooter onNavigate={onNavigate} showToast={showToast} />

      <div className="md:hidden">
        <BottomNav currentTab="cards" onNavigate={onNavigate} />
      </div>
    </div>
  );
};

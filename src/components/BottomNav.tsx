import React from 'react';
import { ScreenId } from '../types';

interface BottomNavProps {
  currentTab: 'wallet' | 'cards' | 'services';
  onNavigate: (tab: 'wallet' | 'cards' | 'services') => void;
}

export const BottomNav: React.FC<BottomNavProps> = ({ currentTab, onNavigate }) => {
  return (
    <nav
      className="fixed bottom-0 w-full z-50 pb-safe pointer-events-none"
      data-active-classes="bg-primary text-on-primary shadow-[0_4px_12px_rgba(0,104,95,0.25)]"
    >
      <div className="max-w-md mx-auto px-space-md pb-space-sm pt-space-xs">
        <div className="pointer-events-auto h-16 rounded-full bg-surface-container-lowest/95 backdrop-blur-xl shadow-[0_10px_25px_-5px_rgba(14,28,47,0.08),0_4px_10px_-4px_rgba(14,28,47,0.04)] px-space-sm flex items-center justify-between">
          {/* Wallet */}
          <a
            aria-current={currentTab === 'wallet' ? 'page' : undefined}
            className={`flex-1 flex items-center justify-center gap-2 h-11 rounded-full transition-all duration-200 group ${
              currentTab === 'wallet'
                ? 'bg-primary text-on-primary shadow-[0_4px_12px_rgba(0,104,95,0.25)]'
                : 'text-on-surface-variant hover:text-on-surface'
            }`}
            data-path="wallet"
            href="#wallet"
            onClick={(e) => {
              e.preventDefault();
              onNavigate('wallet');
            }}
          >
            <span className="material-symbols-outlined text-[22px]">account_balance_wallet</span>
            <span className="font-label-md text-label-md">Wallet</span>
          </a>

          {/* Cards */}
          <a
            aria-current={currentTab === 'cards' ? 'page' : undefined}
            className={`flex-1 flex items-center justify-center gap-2 h-11 rounded-full transition-all duration-200 group ${
              currentTab === 'cards'
                ? 'bg-primary text-on-primary shadow-[0_4px_12px_rgba(0,104,95,0.25)]'
                : 'text-on-surface-variant hover:text-on-surface'
            }`}
            data-path="cards"
            href="#cards"
            onClick={(e) => {
              e.preventDefault();
              onNavigate('cards');
            }}
          >
            <span className="material-symbols-outlined text-[22px]">credit_card</span>
            <span className="font-label-md text-label-md">Cards</span>
          </a>

          {/* Services */}
          <a
            aria-current={currentTab === 'services' ? 'page' : undefined}
            className={`flex-1 flex items-center justify-center gap-2 h-11 rounded-full transition-all duration-200 group ${
              currentTab === 'services'
                ? 'bg-primary text-on-primary shadow-[0_4px_12px_rgba(0,104,95,0.25)]'
                : 'text-on-surface-variant hover:text-on-surface'
            }`}
            data-path="services"
            href="#services"
            onClick={(e) => {
              e.preventDefault();
              onNavigate('services');
            }}
          >
            <span className="material-symbols-outlined text-[22px]">apps</span>
            <span className="font-label-md text-label-md">Services</span>
          </a>
        </div>
      </div>
    </nav>
  );
};

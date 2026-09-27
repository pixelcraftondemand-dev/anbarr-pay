/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 */

import React, { useState } from 'react';
import { ScreenId, TransferData } from './types';
import { WalletHomeScreen } from './components/screens/WalletHomeScreen';
import { ServicesScreen } from './components/screens/ServicesScreen';
import { CardsScreen } from './components/screens/CardsScreen';
import { SendStep1Screen } from './components/screens/SendStep1Screen';
import { SendStep2Screen } from './components/screens/SendStep2Screen';
import { SendStep3Screen } from './components/screens/SendStep3Screen';
import { TransactionDetailsScreen } from './components/screens/TransactionDetailsScreen';

interface ToastState {
  id: number;
  message: string;
  icon: string;
}

export default function App() {
  const [currentScreen, setCurrentScreen] = useState<ScreenId>('wallet');
  const [toast, setToast] = useState<ToastState | null>(null);

  const [transferData, setTransferData] = useState<TransferData>({
    recipientName: 'Abu Bakarr Sesay',
    recipientHandle: '@abusesay',
    recipientPhone: '+232 76 924 810',
    recipientAccount: 'SL-4491-09',
    amount: '1,250.00',
    note: 'Invoice #F-882 Supplies for Lumley branch',
    sourceAccount: 'Main Ledger Account',
    sourceBalance: '48,250.00',
    payoutRail: 'anbarr',
  });

  const showToast = (message: string, icon: string = 'check_circle') => {
    const id = Date.now();
    setToast({ id, message, icon });
    setTimeout(() => {
      setToast((prev) => (prev?.id === id ? null : prev));
    }, 2500);
  };

  const handleNavigate = (screen: ScreenId) => {
    window.scrollTo({ top: 0, behavior: 'instant' });
    setCurrentScreen(screen);
  };

  return (
    <div className="min-h-screen bg-surface text-on-surface antialiased relative">
      {/* Active Screen Rendering */}
      {currentScreen === 'wallet' && (
        <WalletHomeScreen onNavigate={handleNavigate} showToast={showToast} />
      )}

      {currentScreen === 'services' && (
        <ServicesScreen onNavigate={handleNavigate} showToast={showToast} />
      )}

      {currentScreen === 'cards' && (
        <CardsScreen onNavigate={handleNavigate} showToast={showToast} />
      )}

      {currentScreen === 'send-step-1' && (
        <SendStep1Screen
          transferData={transferData}
          setTransferData={setTransferData}
          onNavigate={handleNavigate}
          showToast={showToast}
        />
      )}

      {currentScreen === 'send-step-2' && (
        <SendStep2Screen
          transferData={transferData}
          onNavigate={handleNavigate}
          showToast={showToast}
        />
      )}

      {currentScreen === 'send-step-3' && (
        <SendStep3Screen
          transferData={transferData}
          onNavigate={handleNavigate}
          showToast={showToast}
        />
      )}

      {currentScreen === 'transaction-details' && (
        <TransactionDetailsScreen
          onNavigate={handleNavigate}
          showToast={showToast}
        />
      )}

      {/* Floating System Toast */}
      {toast && (
        <div
          role="status"
          aria-live="polite"
          className="fixed bottom-20 left-1/2 -translate-x-1/2 px-4 py-2.5 rounded-full bg-inverse-surface text-inverse-on-surface shadow-2xl flex items-center gap-2 z-50 animate-fade-in transition-all"
        >
          <span className="material-symbols-outlined text-inverse-primary text-[18px]">
            {toast.icon}
          </span>
          <span className="font-body-sm text-body-sm">{toast.message}</span>
        </div>
      )}
    </div>
  );
}

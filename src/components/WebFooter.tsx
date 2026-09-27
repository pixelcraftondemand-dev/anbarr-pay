import React from 'react';
import { ScreenId } from '../types';

interface WebFooterProps {
  onNavigate: (screen: ScreenId) => void;
  showToast: (msg: string, icon?: string) => void;
}

export const WebFooter: React.FC<WebFooterProps> = ({ onNavigate, showToast }) => {
  return (
    <footer className="w-full border-t border-surface-container-high/80 bg-surface-container-lowest py-10 mt-16 text-on-surface">
      <div className="mx-auto max-w-7xl px-4 sm:px-6 lg:px-8">
        <div className="grid grid-cols-1 md:grid-cols-4 gap-8 pb-8 border-b border-surface-container-high">
          {/* Col 1 */}
          <div className="space-y-3 md:col-span-1">
            <div className="flex items-center gap-2">
              <span className="font-headline-sm text-lg font-bold text-on-surface">
                ANBARR PAY
              </span>
              <span className="px-2 py-0.5 rounded-full bg-primary/10 text-primary font-label-code text-[10px] font-bold">
                SIERRA LEONE
              </span>
            </div>
            <p className="font-body-sm text-xs text-on-surface-variant leading-relaxed">
              Institutional-grade ledger infrastructure for Sierra Leone. Regulated by the Bank of Sierra Leone, providing instant settlement, RTGS rail connectivity, and verified SME commerce.
            </p>
            <div className="flex items-center gap-1.5 text-primary text-xs font-label-code">
              <span className="material-symbols-outlined text-[15px]">verified_user</span>
              <span>Hardware Enclave Tier-3 Security</span>
            </div>
          </div>

          {/* Col 2 */}
          <div className="space-y-2">
            <h4 className="font-headline-sm text-xs uppercase tracking-wider text-on-surface font-bold">
              Core Products
            </h4>
            <ul className="space-y-2 text-xs text-on-surface-variant">
              <li>
                <button
                  type="button"
                  onClick={() => onNavigate('wallet')}
                  className="hover:text-primary transition-colors cursor-pointer text-left"
                >
                  Multi-Account Ledger Vault
                </button>
              </li>
              <li>
                <button
                  type="button"
                  onClick={() => onNavigate('cards')}
                  className="hover:text-primary transition-colors cursor-pointer text-left"
                >
                  SLE Virtual & Contactless Cards
                </button>
              </li>
              <li>
                <button
                  type="button"
                  onClick={() => onNavigate('services')}
                  className="hover:text-primary transition-colors cursor-pointer text-left"
                >
                  Mobile Money Interoperability
                </button>
              </li>
              <li>
                <button
                  type="button"
                  onClick={() => onNavigate('send-step-1')}
                  className="hover:text-primary transition-colors cursor-pointer text-left"
                >
                  Peer-to-Peer Instant Transfer
                </button>
              </li>
            </ul>
          </div>

          {/* Col 3 */}
          <div className="space-y-2">
            <h4 className="font-headline-sm text-xs uppercase tracking-wider text-on-surface font-bold">
              Regulatory & Settlement
            </h4>
            <ul className="space-y-2 text-xs text-on-surface-variant">
              <li>
                <span className="hover:text-primary transition-colors cursor-default">
                  BSL Clearing Rail (SL-RTGS v4.2)
                </span>
              </li>
              <li>
                <span className="hover:text-primary transition-colors cursor-default">
                  NRA Direct Tax Reconciliation
                </span>
              </li>
              <li>
                <span className="hover:text-primary transition-colors cursor-default">
                  Freetown Consensus Node 03
                </span>
              </li>
              <li>
                <span className="hover:text-primary transition-colors cursor-default">
                  EDSA Power Token Settlement
                </span>
              </li>
            </ul>
          </div>

          {/* Col 4 */}
          <div className="space-y-3">
            <h4 className="font-headline-sm text-xs uppercase tracking-wider text-on-surface font-bold">
              Assistance & Audit
            </h4>
            <p className="font-body-sm text-xs text-on-surface-variant">
              Need immediate ledger reconciliation or merchant POS onboarding?
            </p>
            <div className="flex flex-col gap-2">
              <button
                type="button"
                onClick={() => showToast('Connecting to 24/7 Freetown Support...', 'support_agent')}
                className="inline-flex items-center justify-center gap-1.5 px-4 py-2 rounded-lg bg-surface-container-high hover:bg-surface-container-highest text-on-surface text-xs font-semibold transition-colors cursor-pointer"
              >
                <span className="material-symbols-outlined text-[16px]">support_agent</span>
                <span>Freetown Support Desk</span>
              </button>
              <div className="text-[11px] font-label-code text-on-surface-variant">
                Hotline: +232 (22) 222-901 • Freetown CBD
              </div>
            </div>
          </div>
        </div>

        <div className="flex flex-col sm:flex-row items-center justify-between pt-6 text-xs text-on-surface-variant gap-4">
          <p>© 2026 Anbarr Pay Technologies (SL) Ltd. Regulated under Bank of Sierra Leone Financial Institutions Act.</p>
          <div className="flex items-center gap-4 text-xs font-label-code">
            <span>ISO/IEC 27001</span>
            <span>·</span>
            <span>PCI-DSS Level 1</span>
            <span>·</span>
            <span>BSL Reg 24-B</span>
          </div>
        </div>
      </div>
    </footer>
  );
};

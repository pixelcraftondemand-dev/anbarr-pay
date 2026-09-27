import React from 'react';
import { ScreenId } from '../types';

interface WebNavbarProps {
  currentScreen: ScreenId;
  onNavigate: (screen: ScreenId) => void;
  onNotificationClick?: () => void;
  onSupportClick?: () => void;
}

export const BRAND_LOGO =
  'https://lh3.googleusercontent.com/aida/AEtjO1UGd__-8hoE04R0347XS_EhqVj2OOSbN_dcY6FSPsn2bRymV6d6sq40qlKI0Ksn8saYO3PSWiRtSXY72wP5k_kI4XeqKYT__24KYxzssgKYZjypqv0IibRjdY38MIqd_ZCtQS4xoRsMhQ-ZJgQZx0B-wiUxXO1_M9RqKDwmM_srYC558MTYyZCqAm54UdYf1IE9G7HBBucuf74vSaFvdGWRAdTbZPILM3EzUVSYeR-SPZGlXMzSQMHH5Fo';

export const USER_AVATAR =
  'https://lh3.googleusercontent.com/aida-public/AB6AXuDpvLQTVRnSWtaAMEnNWKIknG-h0HrWnknbS99DbUyzOf8guCDms3MKPITk6YhiWLYw7aV2LDpVdBshELuPqXMrI-_F5DHBo0MGEIJuCTCQOEkFqj9ryportbyBFXaOkMsFGwiGQSuq4rIRl2qRMGoJLRq0wCz1REQ5LDx3YbxscbQ3oeWFEuX73TXES4vaHngqdK2SxP-IjIh1HhRe-M7KOnnT0pDLENFWATAEfNbl1ipI9UvUnO2Iow';

export const KADIATU_AVATAR =
  'https://lh3.googleusercontent.com/aida-public/AB6AXuAUl0FQgwc4KN2UVwXXqyWYUNgGHhFrSnPpaLChaB4CVdljbdJbfaaMRa4nXQ2oSwAyD8sd1zZgIIfkaDAazheLMzXHPh3_DwkSdkIN0QBuqiLONjqDi2FHDnO5ygVSWIGF3kbGd52GrCBfPgD1vacxnw9RoRhDbKp1NxGGNiXpzoQtY_xghejPQiRrM6ETPC3OuPvIcVXy1mRFpOK0FXaKWBKkPibf2CV7YhwZfRysUZdUMa9NhWd7Ng';

export const ABU_AVATAR =
  'https://lh3.googleusercontent.com/aida-public/AB6AXuAHI37-JRtu6glw7utbHzTO2zFO-3-FDaBGPtAr1AY7S5SfszdobBYXGSkjNcBRXjrSc9CsJl2S5XVT9L4VCedzGypDDkuLYwskS3R09e2FsNCUXtCP1R-jDSSqDEkBRR8Q9YUMbHmtS5dv09JugDPRGd-iB-FQ_-UxrKPuJ3PwuCEeoDnjxcrZXtK9YggRzgXTiOuZb4dPm2TKCU7-38mW9r6Vj-5mKjQO9-2-4bgNKbQk2kjaqtBxxw';

export const WebNavbar: React.FC<WebNavbarProps> = ({
  currentScreen,
  onNavigate,
  onNotificationClick,
  onSupportClick,
}) => {
  const isWalletActive = currentScreen === 'wallet';
  const isCardsActive = currentScreen === 'cards';
  const isServicesActive = currentScreen === 'services';

  return (
    <header className="sticky top-0 z-50 w-full border-b border-surface-container-high/80 bg-surface/90 backdrop-blur-md">
      <div className="mx-auto flex h-16 max-w-7xl items-center justify-between px-4 sm:px-6 lg:px-8">
        {/* Left: Brand Identity */}
        <div className="flex items-center gap-8">
          <button
            type="button"
            onClick={() => onNavigate('wallet')}
            className="flex items-center gap-3 cursor-pointer text-left focus:outline-none"
          >
            <img
              alt="Anbarr Pay Brand Logo"
              className="h-8 w-auto object-contain"
              src={BRAND_LOGO}
              onError={(e) => {
                e.currentTarget.style.display = 'none';
              }}
            />
            <div className="flex flex-col">
              <span className="font-headline-sm text-lg font-bold tracking-tight text-on-surface">
                ANBARR PAY
              </span>
              <span className="font-label-code text-[10px] uppercase tracking-wider text-on-surface-variant hidden sm:inline">
                Bank of Sierra Leone Regulated
              </span>
            </div>
          </button>

          {/* Location / Status badge */}
          <div className="hidden md:flex items-center gap-1.5 px-3 py-1 rounded-full bg-surface-container-high/70 text-on-surface-variant">
            <span className="material-symbols-outlined text-[15px] text-primary">location_on</span>
            <span className="font-label-code text-label-code font-semibold tracking-wide uppercase">
              Freetown, SL
            </span>
            <span className="mx-1 h-1 w-1 rounded-full bg-outline-variant"></span>
            <span className="flex items-center gap-1 text-[11px] text-primary font-medium">
              <span className="h-1.5 w-1.5 rounded-full bg-primary animate-pulse"></span>
              RTGS Live
            </span>
          </div>
        </div>

        {/* Center: Desktop Navigation Bar with exact spec links */}
        <nav className="flex items-center gap-1 sm:gap-2">
          <a
            href="#wallet"
            data-path="wallet"
            aria-current={isWalletActive ? 'page' : undefined}
            onClick={(e) => {
              e.preventDefault();
              onNavigate('wallet');
            }}
            className={`flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-semibold transition-colors cursor-pointer ${
              isWalletActive
                ? 'bg-primary text-on-primary shadow-sm'
                : 'text-on-surface-variant hover:text-on-surface hover:bg-surface-container-high/50'
            }`}
          >
            <span className="material-symbols-outlined text-[19px]">account_balance_wallet</span>
            <span>Wallet</span>
          </a>

          <a
            href="#cards"
            data-path="cards"
            aria-current={isCardsActive ? 'page' : undefined}
            onClick={(e) => {
              e.preventDefault();
              onNavigate('cards');
            }}
            className={`flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-semibold transition-colors cursor-pointer ${
              isCardsActive
                ? 'bg-primary text-on-primary shadow-sm'
                : 'text-on-surface-variant hover:text-on-surface hover:bg-surface-container-high/50'
            }`}
          >
            <span className="material-symbols-outlined text-[19px]">credit_card</span>
            <span>Cards</span>
          </a>

          <a
            href="#services"
            data-path="services"
            aria-current={isServicesActive ? 'page' : undefined}
            onClick={(e) => {
              e.preventDefault();
              onNavigate('services');
            }}
            className={`flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-semibold transition-colors cursor-pointer ${
              isServicesActive
                ? 'bg-primary text-on-primary shadow-sm'
                : 'text-on-surface-variant hover:text-on-surface hover:bg-surface-container-high/50'
            }`}
          >
            <span className="material-symbols-outlined text-[19px]">apps</span>
            <span>Services</span>
          </a>
        </nav>

        {/* Right: Quick CTA & User Account Profile */}
        <div className="flex items-center gap-3">
          <button
            type="button"
            onClick={() => onNavigate('send-step-1')}
            className="hidden sm:inline-flex items-center gap-1.5 px-4 py-2 rounded-lg bg-primary hover:bg-primary-container text-on-primary text-sm font-semibold shadow-sm transition-all cursor-pointer active:scale-95"
          >
            <span className="material-symbols-outlined text-[18px]">arrow_outward</span>
            <span>Send Money</span>
          </button>

          <div className="h-6 w-px bg-surface-container-highest hidden sm:block"></div>

          <button
            aria-label="Customer Support"
            className="w-10 h-10 flex items-center justify-center rounded-lg text-on-surface-variant hover:text-on-surface hover:bg-surface-container-high/60 transition-colors cursor-pointer"
            type="button"
            onClick={onSupportClick}
          >
            <span className="material-symbols-outlined text-[20px]">support_agent</span>
          </button>

          <button
            aria-label="Notifications"
            className="w-10 h-10 flex items-center justify-center rounded-lg text-on-surface-variant hover:text-on-surface hover:bg-surface-container-high/60 transition-colors relative cursor-pointer"
            type="button"
            onClick={onNotificationClick}
          >
            <span className="material-symbols-outlined text-[20px]">notifications</span>
            <span className="absolute top-2 right-2 w-2 h-2 rounded-full bg-error ring-2 ring-surface"></span>
          </button>

          <div className="flex items-center gap-2.5 pl-1">
            <img
              alt="Profile"
              className="w-9 h-9 rounded-full object-cover ring-2 ring-primary/20"
              src={USER_AVATAR}
              onError={(e) => {
                e.currentTarget.src = KADIATU_AVATAR;
              }}
            />
            <div className="hidden lg:flex flex-col text-left">
              <span className="font-body-md text-xs font-semibold text-on-surface leading-tight">
                Kadiatu Kamara
              </span>
              <span className="font-label-code text-[10px] text-primary">
                Tier 3 Audited
              </span>
            </div>
          </div>
        </div>
      </div>
    </header>
  );
};

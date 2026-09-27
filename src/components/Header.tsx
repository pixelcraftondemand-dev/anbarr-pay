import React from 'react';

interface HeaderProps {
  type: 'tab' | 'stack';
  title?: string;
  onBack?: () => void;
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

export const Header: React.FC<HeaderProps> = ({
  type,
  title = 'Transaction Details',
  onBack,
  onNotificationClick,
  onSupportClick,
}) => {
  return (
    <header className="fixed top-0 left-0 right-0 w-full z-50 bg-surface/85 backdrop-blur-xl pt-safe shadow-[0_1px_8px_rgba(14,28,47,0.03)]">
      <div className="flex flex-col max-w-md mx-auto">
        {/* Status Bar */}
        <div className="flex items-center justify-between px-space-md pt-space-xs text-on-surface h-6">
          <span className="font-ledger-number-md text-label-code text-on-surface">9:41</span>
          <div className="flex items-center gap-space-xs text-on-surface">
            <span className="material-symbols-outlined text-[15px]">signal_cellular_4_bar</span>
            <span className="material-symbols-outlined text-[15px]">wifi</span>
            <span className="material-symbols-outlined text-[17px]">battery_full</span>
          </div>
        </div>

        {/* Navigation Content */}
        {type === 'tab' ? (
          <div className="h-16 px-space-md flex items-center justify-between">
            <div className="flex items-center gap-space-sm">
              <div className="flex items-center gap-space-xs">
                <img
                  alt="Anbarr Pay Brand Logo"
                  className="h-8 w-auto object-contain"
                  src={BRAND_LOGO}
                  onError={(e) => {
                    e.currentTarget.style.display = 'none';
                  }}
                />
                <span className="font-headline-sm text-headline-sm text-on-surface tracking-tight font-bold">
                  ANBARR PAY
                </span>
              </div>
              <div className="flex items-center gap-1 px-2.5 py-1 rounded-full bg-surface-container-high/60 text-on-surface-variant">
                <span className="material-symbols-outlined text-[14px] text-primary">location_on</span>
                <span className="font-label-code text-label-code font-semibold tracking-wide uppercase">
                  Freetown, SL
                </span>
              </div>
            </div>
            <div className="flex items-center gap-space-xs">
              <button
                aria-label="Customer Support"
                className="w-11 h-11 flex items-center justify-center rounded-full text-on-surface-variant hover:text-on-surface hover:bg-surface-container-high/40 transition-colors"
                type="button"
                onClick={onSupportClick}
              >
                <span className="material-symbols-outlined text-[22px]">support_agent</span>
              </button>
              <button
                aria-label="Notifications"
                className="w-11 h-11 flex items-center justify-center rounded-full text-on-surface-variant hover:text-on-surface hover:bg-surface-container-high/40 transition-colors relative"
                type="button"
                onClick={onNotificationClick}
              >
                <span className="material-symbols-outlined text-[22px]">notifications</span>
                <span className="absolute top-2.5 right-2.5 w-2 h-2 rounded-full bg-error ring-2 ring-surface"></span>
              </button>
              <div className="flex items-center pl-1">
                <img
                  alt="Profile"
                  className="w-8 h-8 rounded-full object-cover ring-1 ring-surface-container-high"
                  src={USER_AVATAR}
                  onError={(e) => {
                    e.currentTarget.src = KADIATU_AVATAR;
                  }}
                />
              </div>
            </div>
          </div>
        ) : (
          <div className="h-16 px-space-md flex items-center justify-between">
            <div className="flex items-center gap-space-xs">
              <button
                aria-label="Go back"
                className="w-11 h-11 flex items-center justify-center rounded-full text-on-surface hover:bg-surface-container-high/40 transition-colors"
                onClick={onBack}
                type="button"
              >
                <span className="material-symbols-outlined text-[24px]">arrow_back</span>
              </button>
              <div className="flex items-center gap-space-xs">
                <img
                  alt="Anbarr Pay Brand Logo"
                  className="h-7 w-auto object-contain"
                  src={BRAND_LOGO}
                  onError={(e) => {
                    e.currentTarget.style.display = 'none';
                  }}
                />
                <h1 className="font-headline-sm text-headline-sm text-on-surface tracking-tight">
                  {title}
                </h1>
              </div>
            </div>
            <div className="flex items-center">
              <img
                alt="Profile"
                className="w-8 h-8 rounded-full object-cover ring-1 ring-surface-container-high"
                src={USER_AVATAR}
                onError={(e) => {
                  e.currentTarget.src = KADIATU_AVATAR;
                }}
              />
            </div>
          </div>
        )}
        <div className="hidden">
          <span className="font-headline-md text-headline-md">{title}</span>
        </div>
      </div>
    </header>
  );
};

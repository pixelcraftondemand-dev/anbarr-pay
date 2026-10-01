// Inline stroke icons (prototype style: 1.6–1.9 stroke, round caps). Kept as
// components so every usage is tree-shaken and no icon font/CDN is needed.

interface IconProps {
  size?: number;
  stroke?: string;
}

export function IconHome({ size = 21, stroke = 'currentColor' }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={stroke} strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round">
      <path d="M4 11l8-7 8 7v9a2 2 0 0 1-2 2h-4v-6h-4v6H6a2 2 0 0 1-2-2z" />
    </svg>
  );
}

export function IconActivity({ size = 21, stroke = 'currentColor' }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={stroke} strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round">
      <path d="M3 12h4l2-7 4 14 2-7h6" />
    </svg>
  );
}

export function IconSend({ size = 21, stroke = 'currentColor' }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={stroke} strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round">
      <path d="M5 12h14M13 6l6 6-6 6" />
    </svg>
  );
}

export function IconTopup({ size = 21, stroke = 'currentColor' }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={stroke} strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
      <path d="M12 4.2v10.3" />
      <path d="M7.6 10.8L12 15.2l4.4-4.4" />
      <path d="M4.2 15.6v3a2 2 0 0 0 2 2h11.6a2 2 0 0 0 2-2v-3" />
    </svg>
  );
}

export function IconSecurity({ size = 21, stroke = 'currentColor' }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={stroke} strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round">
      <path d="M12 3l7 3v6c0 5-3.5 7.5-7 9-3.5-1.5-7-4-7-9V6z" />
    </svg>
  );
}

export function IconLock({ size = 17, stroke = 'currentColor' }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={stroke} strokeWidth="1.8">
      <rect x="5" y="11" width="14" height="9" rx="2" />
      <path d="M8 11V7a4 4 0 1 1 8 0v4" />
    </svg>
  );
}

export function IconEye({ size = 13, stroke = '#fff' }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={stroke} strokeWidth="2">
      <path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7-10-7-10-7z" />
      <circle cx="12" cy="12" r="3" />
    </svg>
  );
}

export function IconSendAction({ size = 20, stroke = 'var(--navy-900)' }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={stroke} strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
      <path d="M3 11.5L20 3.5 12.5 20.5 10.2 12.8 3 11.5z" />
      <path d="M10.2 12.8L20 3.5" />
    </svg>
  );
}

export function IconAddMoney({ size = 20, stroke = 'var(--navy-900)' }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={stroke} strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
      <path d="M3.5 8.2A2.2 2.2 0 0 1 5.7 6h11.6a2.2 2.2 0 0 1 2.2 2.2v.8h1a1.5 1.5 0 0 1 1.5 1.5v6a2.2 2.2 0 0 1-2.2 2.2H5.7a2.2 2.2 0 0 1-2.2-2.2z" />
      <path d="M15.2 13.3h4.3M17.35 11.15v4.3" />
    </svg>
  );
}

export function IconRequest({ size = 20, stroke = 'var(--navy-900)' }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={stroke} strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
      <path d="M12 4.2v10.3" />
      <path d="M7.6 10.8L12 15.2l4.4-4.4" />
      <path d="M4.2 15.6v3a2 2 0 0 0 2 2h11.6a2 2 0 0 0 2-2v-3" />
    </svg>
  );
}

export function IconPayBill({ size = 20, stroke = 'var(--navy-900)' }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={stroke} strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
      <path d="M6 3h12v13.2l-1.5 1.4L15 16.2l-1.5 1.4L12 16.2l-1.5 1.4L9 16.2l-1.5 1.4L6 16.2z" />
      <path d="M9 7.4h6M9 10.4h6M9 13.4h3.2" />
    </svg>
  );
}

export function IconClock({ size = 26, stroke = 'var(--gold-800)' }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={stroke} strokeWidth="2">
      <circle cx="12" cy="12" r="9" />
      <path d="M12 7v5l3 3" />
    </svg>
  );
}

export function IconCheck({ size = 26, stroke = 'var(--green-700)' }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={stroke} strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M5 12l5 5 9-11" />
    </svg>
  );
}

export function IconShieldNote({ size = 14, stroke = 'currentColor' }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={stroke} strokeWidth="2">
      <path d="M12 22s8-4.5 8-11V5l-8-3-8 3v6c0 6.5 8 11 8 11z" />
    </svg>
  );
}

export function IconPhone({ size = 16, stroke = 'var(--navy-700)' }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={stroke} strokeWidth="1.8">
      <rect x="7" y="2" width="10" height="20" rx="2" />
    </svg>
  );
}

export function IconVault({ size = 21, stroke = 'currentColor' }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={stroke} strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round">
      <rect x="3.5" y="5" width="17" height="15" rx="2.5" />
      <circle cx="12" cy="12.5" r="3.6" />
      <path d="M12 10.5v-1M12 15.5v-1M14 12.5h-1M11 12.5h-1" />
      <path d="M6 2.5h12" />
    </svg>
  );
}

export function IconPlus({ size = 20, stroke = 'currentColor' }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={stroke} strokeWidth="1.9" strokeLinecap="round">
      <path d="M12 5v14M5 12h14" />
    </svg>
  );
}

export function IconArrow({ size = 20, stroke = 'currentColor' }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={stroke} strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
      <path d="M5 12h13M13 6.5L18.5 12 13 17.5" />
    </svg>
  );
}

export function IconSpark({ size = 18, stroke = 'var(--amber-800)' }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={stroke} strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
      <path d="M12 3l1.7 5.3L19 10l-5.3 1.7L12 17l-1.7-5.3L5 10l5.3-1.7z" />
    </svg>
  );
}

export function IconBackspace({ size = 20, stroke = 'currentColor' }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={stroke} strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
      <path d="M21 5H8l-5 7 5 7h13a1 1 0 0 0 1-1V6a1 1 0 0 0-1-1z" />
      <path d="M12 9l6 6M18 9l-6 6" />
    </svg>
  );
}

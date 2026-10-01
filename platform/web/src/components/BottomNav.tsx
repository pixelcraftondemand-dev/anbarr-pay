import { NavLink } from 'react-router-dom';
import {
  IconActivity,
  IconHome,
  IconSecurity,
  IconSend,
  IconTopup,
  IconVault,
} from './Icons';

/** Primary destinations, shared by the mobile tab bar and the desktop
 *  sidebar so navigation is identical on both (docs/design-system.md: the
 *  shell adapts, the IA doesn't change). */
export const NAV_ITEMS = [
  { to: '/home', label: 'Home', icon: <IconHome /> },
  { to: '/activity', label: 'Activity', icon: <IconActivity /> },
  { to: '/send', label: 'Send', icon: <IconSend /> },
  { to: '/vault', label: 'Vault', icon: <IconVault /> },
  { to: '/topup', label: 'Top-up', icon: <IconTopup /> },
  { to: '/security', label: 'Security', icon: <IconSecurity /> },
];

export function BottomNav() {
  return (
    <nav className="bottomnav" aria-label="Main">
      {NAV_ITEMS.map((it) => (
        <NavLink
          key={it.to}
          to={it.to}
          className={({ isActive }) => `navbtn${isActive ? ' active' : ''}`}
        >
          {it.icon}
          <span>{it.label}</span>
        </NavLink>
      ))}
    </nav>
  );
}

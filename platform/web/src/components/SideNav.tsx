import { NavLink } from 'react-router-dom';
import { NAV_ITEMS } from './BottomNav';

/**
 * Desktop navigation rail (≥1024px). Same destinations as the bottom tab
 * bar; CSS decides which one renders. The footer states the data provenance
 * instead of hiding it — the ledger-backed claim is part of the brand.
 */
export function SideNav() {
  return (
    <nav className="sidenav" aria-label="Main">
      <div className="brand">AmberPay</div>
      {NAV_ITEMS.map((it) => (
        <NavLink
          key={it.to}
          to={it.to}
          className={({ isActive }) => (isActive ? 'active' : '')}
        >
          {it.icon}
          <span>{it.label}</span>
        </NavLink>
      ))}
      <div className="foot">
        Balances come straight from the AmberPay ledger on every load.
      </div>
    </nav>
  );
}

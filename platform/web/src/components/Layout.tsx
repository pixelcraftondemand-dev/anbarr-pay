import { Outlet } from 'react-router-dom';
import { BottomNav } from './BottomNav';
import { SideNav } from './SideNav';
import { useOnline } from '../hooks/useOnline';

/**
 * Adaptive shell: a bottom tab bar on phones, a fixed sidebar on desktop
 * (≥1024px, decided in CSS). The route content renders inside .app-body in
 * both cases; the white-column problem of v1 is gone because the shell
 * stretches with the viewport and the sidebar owns the left rail.
 */
export function Layout() {
  const online = useOnline();

  return (
    <div className="app-shell">
      <SideNav />
      <div className="app-main">
        {!online && (
          <div className="offline-banner" role="status">
            You're offline — you can view, but money movement is disabled until you reconnect.
          </div>
        )}
        <main className="app-body">
          <Outlet />
        </main>
        <BottomNav />
      </div>
    </div>
  );
}

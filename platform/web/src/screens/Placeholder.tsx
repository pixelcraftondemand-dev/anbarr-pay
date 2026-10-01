import type { ReactNode } from 'react';
import { IconSpark } from '../components/Icons';

/**
 * Honest placeholder for journeys spec'd in docs/ux-flows.md but not built
 * yet. Per checklist §73 we never fake financial functionality: an
 * unimplemented screen says so, rather than pretending to work. The styled
 * ring + route home keeps the screen calm instead of a raw text page.
 */
export function Placeholder({ title, note }: { title: string; note?: ReactNode }) {
  return (
    <section className="placeholder px" aria-label={title}>
      <div className="ph-ring">
        <IconSpark size={22} />
      </div>
      <h1>{title}</h1>
      <p>
        {note ?? (
          <>
            This journey is specified in <code>docs/ux-flows.md</code> but is not built yet.
            Nothing here is simulated — when this screen ships, it will talk to the real Core
            API.
          </>
        )}
      </p>
    </section>
  );
}

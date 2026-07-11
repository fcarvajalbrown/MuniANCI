// A non-interactive toolbar pill for features that are planned but not yet
// available (the future "Fuentes oficiales" per-comuna source lookup).
// It renders disabled and explains itself on hover with "Pronto disponible!".

export interface ComingSoonPillProps {
  label: string;
}

export function ComingSoonPill({ label }: ComingSoonPillProps) {
  return (
    <button
      type="button"
      className="search-pill"
      disabled
      aria-disabled="true"
      title="Pronto disponible!"
    >
      <span className="pill-dot" aria-hidden="true" />
      {label}
    </button>
  );
}

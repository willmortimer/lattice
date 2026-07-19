import { BrandMark } from "./BrandMark";

/** Branded hold shown while theme/profile settle and (optionally) for a beat. */
export function StartupSplash() {
  return (
    <div className="startup-splash" role="status" aria-live="polite" aria-label="Loading Lattice">
      <BrandMark size={72} />
      <h1 className="empty-wordmark">Lattice</h1>
    </div>
  );
}

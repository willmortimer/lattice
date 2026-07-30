/**
 * Placeholder slot for T4 (embedding pack download + Lance stale UX).
 * T2 owns the AI settings shell; replace this body without redesigning the section.
 */
export function EmbeddingPackSettings() {
  return (
    <div data-slot="embedding-pack-settings">
      <h2 className="settings-subsection">Embeddings</h2>
      <p className="settings-copy">
        Local embedding pack download and vector freshness status will appear here. Semantic search
        remains available under Search until then.
      </p>
      <div className="diagnostics-card" role="status">
        <strong>Embedding pack</strong>
        <span>Coming soon — T4 will wire pack status and Lance stale actions into this slot.</span>
      </div>
    </div>
  );
}

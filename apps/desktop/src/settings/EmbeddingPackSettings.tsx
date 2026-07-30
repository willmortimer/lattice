/** Slot for embedding pack download + Lance freshness (filled by embedding-card work). */
export function EmbeddingPackSettings() {
  return (
    <div data-slot="embedding-pack-settings">
      <h2 className="settings-subsection">Embeddings</h2>
      <p className="settings-copy">
        Optional local embedding pack and vector freshness status will appear here. Semantic search
        remains available under Search.
      </p>
    </div>
  );
}

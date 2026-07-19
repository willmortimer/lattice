# Search evaluation harness (stub)

Tiny fixture corpus for Lattice hybrid retrieval quality checks.

## Layout

- `corpus/` — small Markdown notes used as a local retrieval fixture
- `queries.yaml` — query stubs (exact, paraphrase, architecture, negative)

## Running

Indexing and hybrid search APIs live in `lattice-index` / `lattice-handlers`.
Use `FakeEmbeddingProvider` for CI; do not download models for these fixtures.

Metric targets (Recall@5/10, MRR, nDCG@10, FTS vs hybrid) are tracked in the
hybrid search implementation plan under `docs/search/`.

# Dataset (Parquet)

Analytical datasets are directory packages of Parquet facts (plus optional
catalog / semantic sidecars) queried locally (DuckDB).

## Example

```text
Metrics.dataset/
├── facts/
│   └── part-000.parquet
└── README.md                  # optional human notes
```

## Agent notes

- Prefer bounded queries and schema/profile tools over loading entire datasets
  into chat context.
- Treat Parquet as the durable analytical form; UI charts are views over it.

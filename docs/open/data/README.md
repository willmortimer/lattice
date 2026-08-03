# Data app (SQLite package)

Mutable relational data lives in a directory package ending in `.data` (or an
equivalent documented package name), with SQLite as the canonical store and
readable manifests for views/forms/interfaces.

## Example

```text
CRM.data/
├── database.sqlite
├── schema.sql                 # when present / exported
├── views/
├── forms/
└── interfaces/
```

## Agent notes

- CSV import lands *into* a data app; CSV is interchange, not the live canonical
  spreadsheet engine.
- Prefer schema-aware tools (SQLite, CLI table commands, MCP schema helpers)
  over scraping UI.

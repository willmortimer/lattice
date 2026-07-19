import type { CsvPreviewResult } from "./csvPreview";

export interface CsvTablePreviewProps {
  preview: Extract<CsvPreviewResult, { ok: true }>;
}

export function CsvTablePreview({ preview }: CsvTablePreviewProps) {
  const colCount = preview.headers.length;
  const empty = colCount === 0 && preview.rows.length === 0;

  return (
    <div className="lattice-csv-preview" role="region" aria-label="CSV table preview">
      {preview.note && (
        <p className="lattice-csv-preview-note" role="note">
          {preview.note}
        </p>
      )}
      {empty ? (
        <p className="lattice-csv-preview-empty">Empty file</p>
      ) : (
        <div className="lattice-csv-preview-scroll">
          <table className="lattice-csv-preview-table">
            <caption className="visually-hidden">
              Tabular preview of CSV or TSV source
            </caption>
            {colCount > 0 && (
              <thead>
                <tr>
                  {preview.headers.map((header, index) => (
                    <th key={`h-${index}`} scope="col">
                      {header || `Column ${index + 1}`}
                    </th>
                  ))}
                </tr>
              </thead>
            )}
            <tbody>
              {preview.rows.map((row, rowIndex) => (
                <tr key={`r-${rowIndex}`}>
                  {row.map((cell, colIndex) => (
                    <td key={`c-${rowIndex}-${colIndex}`}>{cell}</td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

import {
  Button,
  DialogBackdrop,
  DialogPopup,
  DialogPortal,
  DialogRoot,
  DialogTitle,
} from "@lattice/ui";
import { useMemo } from "react";
import {
  TABULAR_IMPORT_FIELD_TYPES,
  fieldTypeLabel,
  normalizeTabularImportFieldType,
  tabularImportReviewTitle,
  type TabularColumnChoice,
  type TabularImportReviewState,
} from "./tabularImport";
import type { FieldType } from "./types";

interface TabularImportReviewDialogProps {
  review: TabularImportReviewState;
  busy: boolean;
  onCancel: () => void;
  onConfirm: () => void;
  onColumnTypeChange: (columnName: string, fieldType: FieldType) => void;
}

export function TabularImportReviewDialog({
  review,
  busy,
  onCancel,
  onConfirm,
  onColumnTypeChange,
}: TabularImportReviewDialogProps) {
  const columnIndexByName = useMemo(() => {
    const map = new Map<string, number>();
    review.preview.columns.forEach((column, index) => {
      map.set(column.name, index);
    });
    return map;
  }, [review.preview.columns]);

  return (
    <DialogRoot open onOpenChange={(open) => !open && !busy && onCancel()}>
      <DialogPortal>
        <DialogBackdrop className="modal-backdrop" />
        <DialogPopup className="modal-panel csv-import-review-panel">
          <DialogTitle id="tabular-import-review-title">
            {tabularImportReviewTitle(review.format)}
          </DialogTitle>
          <p className="modal-copy">
            {review.preview.row_count === 1 ? "1 row" : `${review.preview.row_count} rows`} into{" "}
            <strong>{review.packageName}.data</strong>. Adjust column types before creating the
            package.
          </p>
          <div className="csv-import-review-scroll">
            <table className="csv-import-review-table">
              <thead>
                <tr>
                  <th scope="col">Column</th>
                  <th scope="col">Type</th>
                  <th scope="col">Samples</th>
                </tr>
              </thead>
              <tbody>
                {review.columns.map((column) => {
                  const previewColumn =
                    review.preview.columns[columnIndexByName.get(column.name) ?? -1];
                  return (
                    <TabularImportColumnRow
                      key={column.name}
                      column={column}
                      inferredType={previewColumn?.field_type ?? column.field_type}
                      sampleValues={previewColumn?.sample_values ?? []}
                      disabled={busy}
                      onTypeChange={onColumnTypeChange}
                    />
                  );
                })}
              </tbody>
            </table>
            {review.preview.sample_rows.length > 0 && (
              <>
                <h3 className="csv-import-review-subhead">Preview rows</h3>
                <table className="csv-import-review-table csv-import-review-preview-table">
                  <thead>
                    <tr>
                      {review.columns.map((column) => (
                        <th key={column.name} scope="col">
                          {column.name}
                        </th>
                      ))}
                    </tr>
                  </thead>
                  <tbody>
                    {review.preview.sample_rows.map((row, rowIndex) => (
                      <tr key={`row-${rowIndex}`}>
                        {review.columns.map((column, columnIndex) => (
                          <td key={column.name}>{row[columnIndex] ?? ""}</td>
                        ))}
                      </tr>
                    ))}
                  </tbody>
                </table>
              </>
            )}
          </div>
          <div className="modal-actions">
            <Button onClick={onCancel} disabled={busy}>
              Cancel
            </Button>
            <Button variant="primary" onClick={onConfirm} disabled={busy}>
              {busy ? "Importing…" : "Import"}
            </Button>
          </div>
        </DialogPopup>
      </DialogPortal>
    </DialogRoot>
  );
}

function TabularImportColumnRow({
  column,
  inferredType,
  sampleValues,
  disabled,
  onTypeChange,
}: {
  column: TabularColumnChoice;
  inferredType: string;
  sampleValues: string[];
  disabled: boolean;
  onTypeChange: (columnName: string, fieldType: FieldType) => void;
}) {
  const inferredLabel = fieldTypeLabel(normalizeTabularImportFieldType(inferredType));
  return (
    <tr>
      <th scope="row">{column.name}</th>
      <td>
        <select
          className="csv-import-review-select"
          value={column.field_type}
          disabled={disabled}
          aria-label={`Type for ${column.name}`}
          onChange={(event) => onTypeChange(column.name, event.target.value as FieldType)}
        >
          {TABULAR_IMPORT_FIELD_TYPES.map((option) => (
            <option key={option.value} value={option.value}>
              {option.label}
            </option>
          ))}
        </select>
      </td>
      <td className="csv-import-review-samples">
        {sampleValues.length > 0
          ? sampleValues.map((value) => (
              <span key={value} className="csv-import-review-sample" title={value}>
                {value}
              </span>
            ))
          : <span className="csv-import-review-empty">—</span>}
        <span className="csv-import-review-inferred">inferred {inferredLabel}</span>
      </td>
    </tr>
  );
}

/** @deprecated Use `TabularImportReviewDialog`. */
export const CsvImportReviewDialog = TabularImportReviewDialog;

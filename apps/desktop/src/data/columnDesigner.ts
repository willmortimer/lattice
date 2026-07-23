import type { FieldType, RollupAggregate } from "./types";
import { fieldTypeLabel } from "./recordDetail";

/** Field types exposed in the add-column designer (matches `lattice_data::FieldType`). */
export const COLUMN_FIELD_TYPES: FieldType[] = [
  "text",
  "long_text",
  "integer",
  "decimal",
  "boolean",
  "date",
  "relation",
  "lookup",
  "rollup",
  "formula",
  "enum",
  "multi_enum",
  "attachment",
];

export const ROLLUP_AGGREGATES: RollupAggregate[] = ["count", "sum", "min", "max"];

export function columnFieldTypeOptions(): Array<{ value: FieldType; label: string }> {
  return COLUMN_FIELD_TYPES.map((fieldType) => ({
    value: fieldType,
    label: fieldTypeLabel(fieldType),
  }));
}

export function rollupAggregateOptions(): Array<{ value: RollupAggregate; label: string }> {
  return ROLLUP_AGGREGATES.map((aggregate) => ({
    value: aggregate,
    label: aggregate.charAt(0).toUpperCase() + aggregate.slice(1),
  }));
}

const COLUMN_NAME_PATTERN = /^[A-Za-z_][A-Za-z0-9_]*$/;

export function validateColumnName(name: string, existingNames: string[]): string | null {
  const trimmed = name.trim();
  if (!trimmed) {
    return "Column name is required.";
  }
  if (!COLUMN_NAME_PATTERN.test(trimmed)) {
    return "Use letters, numbers, and underscores; start with a letter or underscore.";
  }
  if (existingNames.some((existing) => existing.toLowerCase() === trimmed.toLowerCase())) {
    return `Column "${trimmed}" already exists.`;
  }
  return null;
}

export function validateRelationTarget(
  fieldType: FieldType,
  relationTable: string | undefined,
  availableTables: string[],
  currentTable: string,
): string | null {
  if (fieldType !== "relation") {
    return null;
  }
  const target = relationTable?.trim();
  if (!target) {
    return "Choose a target table for relation columns.";
  }
  if (target === currentTable) {
    return "Relation columns cannot target the same table.";
  }
  if (!availableTables.includes(target)) {
    return `Table "${target}" was not found in this package.`;
  }
  return null;
}

export function validateLookupSpec(
  fieldType: FieldType,
  lookupRelation: string | undefined,
  lookupField: string | undefined,
  relationColumns: Array<{ name: string; relation_table?: string }>,
  targetFields: string[],
): string | null {
  if (fieldType !== "lookup") {
    return null;
  }
  const relation = lookupRelation?.trim();
  if (!relation) {
    return "Choose a relation column for lookup fields.";
  }
  const source = relationColumns.find((column) => column.name === relation);
  if (!source) {
    return `Relation column "${relation}" was not found on this table.`;
  }
  const field = lookupField?.trim();
  if (!field) {
    return "Choose a field on the related table.";
  }
  if (!targetFields.includes(field)) {
    return `Field "${field}" was not found on the related table.`;
  }
  return null;
}

export function validateRollupSpec(
  fieldType: FieldType,
  rollupRelation: string | undefined,
  rollupAggregate: RollupAggregate | undefined,
  rollupField: string | undefined,
  relationColumns: Array<{ name: string; relation_table?: string }>,
  targetFields: string[],
  numericTargetFields: string[],
): string | null {
  if (fieldType !== "rollup") {
    return null;
  }
  const relation = rollupRelation?.trim();
  if (!relation) {
    return "Choose a relation column for rollup fields.";
  }
  const source = relationColumns.find((column) => column.name === relation);
  if (!source) {
    return `Relation column "${relation}" was not found on this table.`;
  }
  if (!rollupAggregate) {
    return "Choose a rollup aggregate.";
  }
  const requiresField = rollupAggregate !== "count";
  const field = rollupField?.trim();
  if (requiresField) {
    if (!field) {
      return "Choose a numeric field on the related table.";
    }
    if (!numericTargetFields.includes(field)) {
      return `Field "${field}" must be integer or decimal for ${rollupAggregate}.`;
    }
  } else if (field && !targetFields.includes(field)) {
    return `Field "${field}" was not found on the related table.`;
  }
  return null;
}

export function validateFormulaSpec(
  fieldType: FieldType,
  formula: string | undefined,
  existingNames: string[],
): string | null {
  if (fieldType !== "formula") {
    return null;
  }
  const expression = formula?.trim();
  if (!expression) {
    return "Enter a formula expression (e.g. {price} * {quantity}).";
  }
  const refs = expression.matchAll(/\{([A-Za-z_][A-Za-z0-9_]*)\}/g);
  for (const match of refs) {
    const ref = match[1];
    if (!existingNames.includes(ref)) {
      return `Formula references missing column "${ref}".`;
    }
  }
  return null;
}

export function parseEnumOptionsText(text: string): string[] {
  return text
    .split(/[\n,]/)
    .map((part) => part.trim())
    .filter(Boolean);
}

export function validateEnumOptions(
  fieldType: FieldType,
  optionsText: string | undefined,
): string | null {
  if (fieldType !== "enum" && fieldType !== "multi_enum") {
    return null;
  }
  const options = parseEnumOptionsText(optionsText ?? "");
  if (options.length === 0) {
    return "Enter at least one option (comma or newline separated).";
  }
  const seen = new Set<string>();
  for (const option of options) {
    if (seen.has(option)) {
      return `Duplicate option "${option}".`;
    }
    seen.add(option);
  }
  return null;
}

export interface AddColumnPayload {
  name: string;
  field_type: FieldType;
  relation_table?: string;
  lookup_relation?: string;
  lookup_field?: string;
  rollup_relation?: string;
  rollup_aggregate?: RollupAggregate;
  rollup_field?: string;
  formula?: string;
  options?: string[];
}

export function buildAddColumnPayload(
  name: string,
  fieldType: FieldType,
  relationTable: string | undefined,
  lookupRelation?: string,
  lookupField?: string,
  rollupRelation?: string,
  rollupAggregate?: RollupAggregate,
  rollupField?: string,
  formula?: string,
  optionsText?: string,
): AddColumnPayload {
  const trimmed = name.trim();
  if (fieldType === "relation") {
    return {
      name: trimmed,
      field_type: fieldType,
      relation_table: relationTable?.trim(),
    };
  }
  if (fieldType === "lookup") {
    return {
      name: trimmed,
      field_type: fieldType,
      lookup_relation: lookupRelation?.trim(),
      lookup_field: lookupField?.trim(),
    };
  }
  if (fieldType === "rollup") {
    const field = rollupField?.trim();
    return {
      name: trimmed,
      field_type: fieldType,
      rollup_relation: rollupRelation?.trim(),
      rollup_aggregate: rollupAggregate,
      ...(field ? { rollup_field: field } : {}),
    };
  }
  if (fieldType === "formula") {
    return {
      name: trimmed,
      field_type: fieldType,
      formula: formula?.trim(),
    };
  }
  if (fieldType === "enum" || fieldType === "multi_enum") {
    return {
      name: trimmed,
      field_type: fieldType,
      options: parseEnumOptionsText(optionsText ?? ""),
    };
  }
  return {
    name: trimmed,
    field_type: fieldType,
  };
}

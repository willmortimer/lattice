export const DATA_TABLE_GRID_PANEL_ID = "grid";
export const DATA_TABLE_DETAIL_PANEL_ID = "detail";
export const DATA_TABLE_SPLIT_GROUP_ID = "data-table-record-split";

export type DataTablePanelSizes = {
  table: number;
  detail: number;
};

export const DEFAULT_DATA_TABLE_PANEL_SIZES: DataTablePanelSizes = {
  table: 62,
  detail: 38,
};

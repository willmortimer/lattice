/** Request sent from the main thread to the Shiki worker. */
export interface ShikiHighlightRequest {
  id: number;
  code: string;
  lang: string;
}

/** Successful highlight response from the worker. */
export interface ShikiHighlightSuccess {
  id: number;
  ok: true;
  html: string;
}

/** Failed highlight response from the worker. */
export interface ShikiHighlightFailure {
  id: number;
  ok: false;
  error: string;
}

export type ShikiHighlightResponse = ShikiHighlightSuccess | ShikiHighlightFailure;

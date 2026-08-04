import { useEffect, useState } from "react";
import type * as Y from "yjs";

import { getCommentsMap, listStickyComments, type StickyComment } from "./commentStore";

/** Subscribe to the comments Y.Map and return a snapshot list. */
export function useStickyComments(ydoc: Y.Doc | null): StickyComment[] {
  const [comments, setComments] = useState<StickyComment[]>(() =>
    ydoc ? listStickyComments(ydoc) : [],
  );

  useEffect(() => {
    if (!ydoc) {
      setComments([]);
      return;
    }

    const refresh = () => setComments(listStickyComments(ydoc));
    refresh();

    const map = getCommentsMap(ydoc);
    map.observeDeep(refresh);
    return () => {
      map.unobserveDeep(refresh);
    };
  }, [ydoc]);

  return comments;
}

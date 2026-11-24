/** Quick-open over the project's lab records: a scalable alternative to browsing the explorer. */

import { useCallback, useMemo } from "react";
import { rankByQuery, SearchModal } from "imp-graph-view";
import "imp-graph-view/styles.css";
import { recordsList, type ProjectInfo, type RecordSummary } from "./records";

/** Most results listed at once; narrowing the query reaches the rest. */
const RESULT_LIMIT = 50;

const KIND_LABEL: Record<string, string> = {
  "imp-graph": "graph",
  notation: "notation",
  other: "node",
};

type RecordSearchProps = {
  open: boolean;
  onClose: () => void;
  project: ProjectInfo | null;
  onOpen: (record: RecordSummary) => void;
};

/** Newest first; records without a modified time last. */
function byRecent(a: RecordSummary, b: RecordSummary): number {
  return (b.modifiedAt ?? "").localeCompare(a.modifiedAt ?? "");
}

export function RecordSearch({ open, onClose, project, onOpen }: RecordSearchProps) {
  const storeLabels = useMemo(
    () => new Map((project?.stores ?? []).map((s) => [s.id, s.label])),
    [project],
  );

  const search = useCallback(
    async (query: string): Promise<RecordSummary[]> => {
      if (!project) return [];
      const text = query.trim();
      // The host filters by title substring; ranking puts prefix and word-start hits first.
      const records = await recordsList({ kind: "lab", text: text || null });
      const ranked = text
        ? rankByQuery(records, text, (r) => [r.title, r.nodeId])
        : [...records].sort(byRecent);
      return ranked.slice(0, RESULT_LIMIT);
    },
    [project],
  );

  return (
    <SearchModal
      open={open}
      onClose={onClose}
      search={search}
      getKey={(r) => r.nodeId}
      ariaLabel="Go to record"
      placeholder="Go to record…"
      emptyText={(query) =>
        !project
          ? "Open a project to search its records"
          : query.trim()
            ? "No matching records"
            : "No records in this project"
      }
      renderItem={(r) => {
        const store = r.corpusId ? (storeLabels.get(r.corpusId) ?? r.corpusId) : null;
        return (
          <>
            <span className="imp-gv-search-item-label">{r.title || r.nodeId}</span>
            <span className="imp-gv-search-item-detail">
              {[KIND_LABEL[r.kind] ?? r.kind, store].filter(Boolean).join(" · ")}
            </span>
          </>
        );
      }}
      onActivate={(r) => onOpen(r)}
    />
  );
}

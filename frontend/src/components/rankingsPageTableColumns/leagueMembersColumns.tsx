import type { Column } from "@/components/common/RankingTable/types";
import type { LeagueMember } from "@/features/draft/types";

interface LeagueMembersColumnsOptions {
  editingTeamId: number | null;
  editingTeamName: string;
  onEditingTeamNameChange: (name: string) => void;
  onStartEdit: (teamId: number, currentName: string) => void;
  onSaveEdit: (teamId: number) => void;
  onCancelEdit: () => void;
  onRemove: (member: LeagueMember) => void;
}

/** Column set for the members table on the league settings page. */
export function getLeagueMembersColumns({
  editingTeamId,
  editingTeamName,
  onEditingTeamNameChange,
  onStartEdit,
  onSaveEdit,
  onCancelEdit,
  onRemove,
}: LeagueMembersColumnsOptions): Column<LeagueMember>[] {
  return [
    { key: "draftOrder", header: "Order" },
    {
      key: "displayName",
      header: "Player",
      render: (value: string | undefined) => (
        <span className="text-gray-900 whitespace-nowrap">{value ?? "Unknown"}</span>
      ),
    },
    {
      key: "teamName",
      header: "Team",
      render: (value: string | undefined, m) =>
        editingTeamId === m.fantasyTeamId ? (
          <div className="flex items-center gap-2">
            <input
              type="text"
              value={editingTeamName}
              onChange={(e) => onEditingTeamNameChange(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && onSaveEdit(m.fantasyTeamId)}
              className="px-2 py-1 border border-gray-300 rounded-none text-sm w-32"
              autoFocus
            />
            <button
              onClick={() => onSaveEdit(m.fantasyTeamId)}
              className="text-xs text-green-600 font-bold"
            >
              Save
            </button>
            <button onClick={onCancelEdit} className="text-xs text-gray-400">
              Cancel
            </button>
          </div>
        ) : (
          <button
            onClick={() => onStartEdit(m.fantasyTeamId, value ?? "")}
            className="text-gray-700 hover:text-[#2563EB] cursor-pointer"
            title="Click to edit"
          >
            {value ?? "-"}
          </button>
        ),
    },
    {
      key: "actions",
      header: "Actions",
      render: (_value, m) => (
        <button
          onClick={() => onRemove(m)}
          className="text-red-400 hover:text-red-600 transition-colors"
          title="Remove member"
          aria-label={`Remove ${m.displayName ?? "member"}`}
        >
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"
            />
          </svg>
        </button>
      ),
    },
  ];
}

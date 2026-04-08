interface GameTabsProps {
  activeTab: string;
  setActiveTab: (tab: string) => void;
  hasFantasyTeams?: boolean;
  hasExtendedData?: boolean;
}

export default function GameTabs({
  activeTab,
  setActiveTab,
  hasFantasyTeams,
  hasExtendedData,
}: GameTabsProps) {
  const tabClass = (tab: string) =>
    `py-3 px-6 font-bold uppercase tracking-wider text-sm border-2 border-b-0 transition-colors duration-100 ${
      activeTab === tab
        ? "bg-[#1A1A1A] text-white border-[#1A1A1A]"
        : "bg-white text-gray-500 border-transparent hover:text-[#1A1A1A]"
    }`;

  return (
    <div className="border-b-2 border-[#1A1A1A] mb-4 flex">
      <button onClick={() => setActiveTab("games")} className={tabClass("games")}>
        NHL Games
      </button>
      {(hasFantasyTeams || hasExtendedData) && (
        <button onClick={() => setActiveTab("fantasy")} className={tabClass("fantasy")}>
          My League
        </button>
      )}
      {hasExtendedData && (
        <button onClick={() => setActiveTab("matchups")} className={tabClass("matchups")}>
          Player Matchups
        </button>
      )}
    </div>
  );
}

import type { MyTeamDiagnosis } from "@/features/pulse";

interface Props {
  data: MyTeamDiagnosis;
}

/**
 * "Your Read" block on Pulse: rank/gap strip, concentration-by-team
 * chips, and the descriptive diagnosis narrative (Where You Stand /
 * Player-by-Player / What to Expect). The per-player breakdown table
 * lives in a sibling `RosterBreakdownSection`.
 */
export default function YourReadSection({ data }: Props) {
  const { diagnosis } = data;
  const hasNarrative = diagnosis.narrativeMarkdown.trim().length > 0;

  return (
    <section className="bg-white border-2 border-[#1A1A1A] overflow-hidden">
      <header className="bg-[var(--color-you)] px-6 py-3">
        <h2 className="font-extrabold uppercase tracking-wider text-sm text-[#1A1A1A]">
          Your Read
        </h2>
      </header>

      <div className="p-6 space-y-4">
        <RankStrip data={data} />
        <ConcentrationStrip data={data} />
        <div className="pt-3 border-t border-gray-200">
          {hasNarrative ? (
            <MarkdownBlock text={diagnosis.narrativeMarkdown} />
          ) : (
            <StaticFallback data={data} />
          )}
        </div>
      </div>
    </section>
  );
}

function RankStrip({ data }: { data: MyTeamDiagnosis }) {
  const { diagnosis } = data;
  const down = diagnosis.gapToFirst;
  const aheadOfThird =
    diagnosis.gapToThird >= 0 ? diagnosis.gapToThird : -diagnosis.gapToThird;
  const aheadOrBehindLabel = diagnosis.gapToThird >= 0 ? "ahead of 3rd" : "behind 3rd";
  return (
    <div className="flex flex-wrap items-baseline gap-x-6 gap-y-1 font-bold uppercase tracking-wider">
      <span className="text-2xl">
        #{diagnosis.leagueRank} / {diagnosis.leagueSize}
      </span>
      <span className="text-sm text-gray-700">
        {down > 0 ? `down ${down} to 1st` : "leading 1st"}
      </span>
      <span className="text-xs text-gray-500">
        {aheadOfThird} pts {aheadOrBehindLabel}
      </span>
    </div>
  );
}

function ConcentrationStrip({ data }: { data: MyTeamDiagnosis }) {
  const concentration = data.diagnosis.concentrationByTeam;
  if (concentration.length === 0) return null;
  return (
    <div>
      <div className="text-[10px] font-bold uppercase tracking-wider text-gray-500 mb-2">
        Roster concentration
      </div>
      <div className="flex flex-wrap gap-2">
        {concentration.map((c) => (
          <div
            key={c.nhlTeam}
            className="border-2 border-[#1A1A1A] px-2 py-1 text-xs font-bold tracking-wider"
          >
            <span className="uppercase">{c.nhlTeam}</span>
            <span className="text-gray-500"> · </span>
            <span>{c.rostered}p</span>
            <span className="text-gray-500"> · </span>
            <span>{c.teamPlayoffPoints} pts</span>
          </div>
        ))}
      </div>
    </div>
  );
}

function StaticFallback({ data }: { data: MyTeamDiagnosis }) {
  const top = data.diagnosis.concentrationByTeam[0];
  return (
    <div className="text-sm leading-relaxed text-gray-800 space-y-2">
      <p>
        Ranked {data.diagnosis.leagueRank} of {data.diagnosis.leagueSize}, {data.diagnosis.gapToFirst}
        {" "}points behind 1st. Largest stack is <strong>{top?.nhlTeam ?? "—"}</strong>
        {top ? ` (${top.rostered} rostered, ${top.teamPlayoffPoints} pts)` : ""}.
      </p>
      <p className="text-xs text-gray-500">
        Narrative unavailable — diagnosis generator is off or has not run for this team today.
      </p>
    </div>
  );
}

function MarkdownBlock({ text }: { text: string }) {
  type Block =
    | { kind: "heading"; text: string }
    | { kind: "paragraph"; text: string }
    | { kind: "list"; items: string[] };

  const blocks: Block[] = [];
  let paragraph: string[] = [];
  let list: string[] = [];
  const flushParagraph = () => {
    if (paragraph.length > 0) {
      blocks.push({ kind: "paragraph", text: paragraph.join(" ").trim() });
      paragraph = [];
    }
  };
  const flushList = () => {
    if (list.length > 0) {
      blocks.push({ kind: "list", items: list });
      list = [];
    }
  };
  for (const rawLine of text.split("\n")) {
    const line = rawLine.trim();
    // Claude occasionally emits markdown horizontal rules as section
    // separators ("---", "***", "___"). The visual card already has
    // an H3 + border for each section — the rule is noise; drop it.
    if (/^[-*_]{3,}\s*$/.test(line)) {
      flushParagraph();
      flushList();
      continue;
    }
    if (line === "") {
      flushParagraph();
      flushList();
      continue;
    }
    if (line.startsWith("### ")) {
      flushParagraph();
      flushList();
      blocks.push({ kind: "heading", text: line.slice(4).trim() });
      continue;
    }
    if (line.startsWith("- ")) {
      flushParagraph();
      list.push(line.slice(2).trim());
      continue;
    }
    flushList();
    paragraph.push(line);
  }
  flushParagraph();
  flushList();

  return (
    <div className="space-y-3">
      {blocks.map((b, i) => {
        if (b.kind === "heading") {
          return (
            <h3
              key={i}
              className={`font-extrabold uppercase tracking-wider text-xs text-[#1A1A1A] ${
                i === 0 ? "" : "pt-3 border-t border-gray-200"
              }`}
            >
              {b.text}
            </h3>
          );
        }
        if (b.kind === "list") {
          return (
            <ul
              key={i}
              className="list-disc pl-5 space-y-1 text-sm leading-relaxed text-gray-800"
            >
              {b.items.map((item, j) => (
                <li key={j}>{renderBold(item)}</li>
              ))}
            </ul>
          );
        }
        return (
          <p key={i} className="text-sm leading-relaxed text-gray-800">
            {renderBold(b.text)}
          </p>
        );
      })}
    </div>
  );
}

function renderBold(text: string): React.ReactNode {
  const parts = text.split(/(\*\*[^*]+\*\*)/g);
  return parts.map((part, i) => {
    if (part.startsWith("**") && part.endsWith("**")) {
      return <strong key={i}>{part.slice(2, -2)}</strong>;
    }
    return <span key={i}>{part}</span>;
  });
}

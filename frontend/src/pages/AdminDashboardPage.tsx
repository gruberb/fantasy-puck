import { useState } from "react";
import { Link, Navigate } from "react-router-dom";
import { useAuth } from "@/contexts/AuthContext";
import LoadingSpinner from "@/components/common/LoadingSpinner";
import PageHeader from "@/components/common/PageHeader";
import { APP_CONFIG } from "@/config";
import { formatSeason } from "@/utils/format";
import { useLeagues } from "@/features/draft";
import {
  AdminActionCard,
  ConfirmDialog,
  adminApi,
  useAdminAction,
  type CacheScope,
  type CalibrationReport,
  type SweepParams,
} from "@/features/admin";

/**
 * Admin-only dashboard. Every panel wraps one `/api/admin/*` endpoint.
 * Result state is per-panel and lives in the browser session; the
 * backend already enforces `is_admin`, so we also hide the route for
 * non-admins here for good UX.
 */
const AdminDashboardPage = () => {
  const { user, profile, loading: authLoading } = useAuth();

  if (authLoading) return <LoadingSpinner message="Checking access..." />;
  if (!user) return <Navigate to="/" replace />;
  if (!profile?.isAdmin) return <Navigate to="/my-leagues" replace />;

  return <AdminDashboardInner userId={user.id} />;
};

function AdminDashboardInner({ userId }: { userId: string }) {
  const { leagues, loading: leaguesLoading } = useLeagues(userId, true);
  const today = new Date().toISOString().slice(0, 10);

  return (
    <div className="space-y-8">
      <PageHeader
        title="Admin"
        subtitle="Cache, calibration, ingest, and league administration."
        badge="Admin"
      />

      {/* Routine operations — cache and data warmup. */}
      <section className="space-y-4">
        <SectionHeader label="Cache + Warmup" />
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
          <InvalidateCachePanel defaultDate={today} />
          <PrewarmPanel />
          <ProcessRankingsPanel defaultDate={today} />
          <RehydratePanel />
        </div>
      </section>

      {/* Historical ingest — rarely run, typically during season prep. */}
      <section className="space-y-4">
        <SectionHeader label="Historical Ingest" />
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
          <BackfillHistoricalPanel />
          <RebackfillCarouselPanel />
        </div>
      </section>

      {/* Prediction-model diagnostics. */}
      <section className="space-y-4">
        <SectionHeader label="Model Calibration" />
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
          <CalibratePanel />
          <CalibrateSweepPanel />
        </div>
      </section>

      {/* All leagues — cross-league admin view. */}
      <section className="space-y-4">
        <SectionHeader label="All Leagues" />
        <div className="bg-white border-2 border-[#1A1A1A] p-5">
          {leaguesLoading ? (
            <LoadingSpinner size="small" message="Loading leagues..." />
          ) : leagues.length === 0 ? (
            <p className="text-sm text-gray-500">No leagues exist yet.</p>
          ) : (
            <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
              {leagues.map((league) => (
                <Link
                  key={league.id}
                  to={`/league/${league.id}/settings`}
                  className="group block p-4 border-2 border-[#1A1A1A] bg-white hover:bg-[#FACC15] transition-colors"
                >
                  <p className="font-extrabold text-[#1A1A1A] uppercase tracking-wider text-sm">
                    {league.name}
                  </p>
                  <p className="text-[11px] text-gray-500 mt-1">
                    {formatSeason(league.season)}
                  </p>
                </Link>
              ))}
            </div>
          )}
        </div>
      </section>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Panels
// ---------------------------------------------------------------------------

function SectionHeader({ label }: { label: string }) {
  return (
    <h2 className="font-extrabold uppercase tracking-widest text-xs text-[#1A1A1A] border-b-2 border-[#1A1A1A] pb-1">
      {label}
    </h2>
  );
}

function InvalidateCachePanel({ defaultDate }: { defaultDate: string }) {
  const [mode, setMode] = useState<CacheScope>("today");
  const [date, setDate] = useState(defaultDate);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const action = useAdminAction(adminApi.invalidateCache);

  const scope = mode === "all" || mode === "today" ? mode : date;
  const needsConfirm = mode === "all";

  const submit = () => {
    if (needsConfirm) {
      setConfirmOpen(true);
      return;
    }
    void action.run(scope);
  };

  return (
    <>
      <AdminActionCard
        title="Invalidate Cache"
        description="Flush cached narratives, insights, race-odds, and match-day payloads. Use `today` after a prompt change to force re-generation; `all` wipes the table."
        onRun={submit}
        data={action.data}
        error={action.error}
        ranAt={action.ranAt}
        isPending={action.isPending}
      >
        <label className="flex flex-col gap-1 text-[10px] uppercase tracking-widest font-bold text-gray-500">
          Scope
          <select
            value={mode === "all" || mode === "today" ? mode : "date"}
            onChange={(e) => setMode(e.target.value as CacheScope)}
            className="border-2 border-[#1A1A1A] px-2 py-1.5 text-sm font-bold text-[#1A1A1A]"
          >
            <option value="today">Today</option>
            <option value="all">All (destructive)</option>
            <option value="date">Specific date…</option>
          </select>
        </label>
        {mode !== "all" && mode !== "today" && (
          <label className="flex flex-col gap-1 text-[10px] uppercase tracking-widest font-bold text-gray-500">
            Date
            <input
              type="date"
              value={date}
              onChange={(e) => setDate(e.target.value)}
              className="border-2 border-[#1A1A1A] px-2 py-1.5 text-sm font-bold text-[#1A1A1A]"
            />
          </label>
        )}
      </AdminActionCard>
      <ConfirmDialog
        open={confirmOpen}
        title="Wipe all cached responses?"
        body="This deletes every row in `response_cache`. The next request for each endpoint will regenerate from scratch — Claude calls will re-run on the next Pulse/Insights hit."
        confirmLabel="Invalidate All"
        onConfirm={() => {
          setConfirmOpen(false);
          void action.run("all");
        }}
        onCancel={() => setConfirmOpen(false)}
      />
    </>
  );
}

function ProcessRankingsPanel({ defaultDate }: { defaultDate: string }) {
  const [date, setDate] = useState(defaultDate);
  const action = useAdminAction(adminApi.processRankings);

  return (
    <AdminActionCard
      title="Reprocess Daily Rankings"
      description="Re-run the rankings rollup for a given date across every league. Safe to re-run; overwrites the daily_rankings row for each team."
      onRun={() => void action.run(date)}
      data={action.data}
      error={action.error}
      ranAt={action.ranAt}
      isPending={action.isPending}
    >
      <label className="flex flex-col gap-1 text-[10px] uppercase tracking-widest font-bold text-gray-500">
        Date
        <input
          type="date"
          value={date}
          onChange={(e) => setDate(e.target.value)}
          className="border-2 border-[#1A1A1A] px-2 py-1.5 text-sm font-bold text-[#1A1A1A]"
        />
      </label>
    </AdminActionCard>
  );
}

function PrewarmPanel() {
  const action = useAdminAction(adminApi.prewarm);
  return (
    <AdminActionCard
      title="Prewarm Caches"
      description="Fire-and-forget: queue background jobs to refresh the Edge mirror and regenerate insights + race-odds for every league. Returns immediately."
      onRun={() => void action.run()}
      data={action.data}
      error={action.error}
      ranAt={action.ranAt}
      isPending={action.isPending}
    />
  );
}

function RehydratePanel() {
  const [confirmOpen, setConfirmOpen] = useState(false);
  const action = useAdminAction(adminApi.rehydrate);

  return (
    <>
      <AdminActionCard
        title="Rehydrate NHL Mirror"
        description="Run every NHL mirror poller once and backfill nhl_player_game_stats from each game. Long-running — several minutes on cold cache."
        onRun={() => setConfirmOpen(true)}
        data={action.data}
        error={action.error}
        ranAt={action.ranAt}
        isPending={action.isPending}
      />
      <ConfirmDialog
        open={confirmOpen}
        title="Rehydrate the NHL mirror?"
        body="This pulls every game and boxscore for the current season. Typically runs in a few minutes but pins a worker while it's going. Proceed?"
        confirmLabel="Rehydrate"
        onConfirm={() => {
          setConfirmOpen(false);
          void action.run();
        }}
        onCancel={() => setConfirmOpen(false)}
      />
    </>
  );
}

function BackfillHistoricalPanel() {
  const [start, setStart] = useState("2025-04-19");
  const [end, setEnd] = useState("2025-06-30");
  const action = useAdminAction(adminApi.backfillHistorical);

  return (
    <AdminActionCard
      title="Backfill Historical Playoffs"
      description="Ingest completed playoff games between two dates into playoff_game_results + playoff_skater_game_stats. Used once per historical season."
      onRun={() => void action.run(start, end)}
      data={action.data}
      error={action.error}
      ranAt={action.ranAt}
      isPending={action.isPending}
    >
      <div className="grid grid-cols-2 gap-2">
        <label className="flex flex-col gap-1 text-[10px] uppercase tracking-widest font-bold text-gray-500">
          Start
          <input
            type="date"
            value={start}
            onChange={(e) => setStart(e.target.value)}
            className="border-2 border-[#1A1A1A] px-2 py-1.5 text-sm font-bold text-[#1A1A1A]"
          />
        </label>
        <label className="flex flex-col gap-1 text-[10px] uppercase tracking-widest font-bold text-gray-500">
          End
          <input
            type="date"
            value={end}
            onChange={(e) => setEnd(e.target.value)}
            className="border-2 border-[#1A1A1A] px-2 py-1.5 text-sm font-bold text-[#1A1A1A]"
          />
        </label>
      </div>
    </AdminActionCard>
  );
}

function RebackfillCarouselPanel() {
  const [season, setSeason] = useState(APP_CONFIG.DEFAULT_SEASON);
  const action = useAdminAction(adminApi.rebackfillCarousel);
  const options = seasonOptions();

  return (
    <AdminActionCard
      title="Rebackfill Carousel"
      description="Re-ingest a playoff season via the carousel endpoint. Fixes missing Cup Final games when the upstream JSON lagged the actual schedule."
      onRun={() => void action.run(season)}
      data={action.data}
      error={action.error}
      ranAt={action.ranAt}
      isPending={action.isPending}
    >
      <label className="flex flex-col gap-1 text-[10px] uppercase tracking-widest font-bold text-gray-500">
        Season
        <select
          value={season}
          onChange={(e) => setSeason(e.target.value)}
          className="border-2 border-[#1A1A1A] px-2 py-1.5 text-sm font-bold text-[#1A1A1A]"
        >
          {options.map((s) => (
            <option key={s} value={s}>
              {formatSeason(s)}
            </option>
          ))}
        </select>
      </label>
    </AdminActionCard>
  );
}

function CalibratePanel() {
  const [season, setSeason] = useState(APP_CONFIG.DEFAULT_SEASON);
  const action = useAdminAction(adminApi.calibrate);
  const options = seasonOptions();

  return (
    <AdminActionCard
      title="Calibrate"
      description="Score the race-odds model against realized playoff outcomes for the selected season. Returns Brier + log-loss per round and per-team predicted-vs-actual deltas."
      onRun={() => void action.run(season)}
      data={action.data}
      error={action.error}
      ranAt={action.ranAt}
      isPending={action.isPending}
      summary={action.data ? <CalibrateSummary report={action.data as CalibrationReport} /> : null}
    >
      <label className="flex flex-col gap-1 text-[10px] uppercase tracking-widest font-bold text-gray-500">
        Season
        <select
          value={season}
          onChange={(e) => setSeason(e.target.value)}
          className="border-2 border-[#1A1A1A] px-2 py-1.5 text-sm font-bold text-[#1A1A1A]"
        >
          {options.map((s) => (
            <option key={s} value={s}>
              {formatSeason(s)}
            </option>
          ))}
        </select>
      </label>
    </AdminActionCard>
  );
}

function CalibrateSweepPanel() {
  const [season, setSeason] = useState(APP_CONFIG.DEFAULT_SEASON);
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [params, setParams] = useState<SweepParams>({});
  const action = useAdminAction(adminApi.calibrateSweep);
  const options = seasonOptions();

  return (
    <>
      <AdminActionCard
        title="Calibrate Sweep"
        description="Grid-search hyperparameters against the selected season. Grid is capped at 200 cells on the backend; can take minutes. Leave advanced fields blank for the default grid."
        onRun={() => setConfirmOpen(true)}
        data={action.data}
        error={action.error}
        ranAt={action.ranAt}
        isPending={action.isPending}
      >
        <label className="flex flex-col gap-1 text-[10px] uppercase tracking-widest font-bold text-gray-500">
          Season
          <select
            value={season}
            onChange={(e) => setSeason(e.target.value)}
            className="border-2 border-[#1A1A1A] px-2 py-1.5 text-sm font-bold text-[#1A1A1A]"
          >
            {options.map((s) => (
              <option key={s} value={s}>
                {formatSeason(s)}
              </option>
            ))}
          </select>
        </label>
        <button
          type="button"
          onClick={() => setAdvancedOpen((v) => !v)}
          className="text-[10px] uppercase tracking-widest font-bold text-[#2563EB] underline decoration-dotted"
        >
          {advancedOpen ? "Hide advanced grid" : "Advanced grid"}
        </button>
        {advancedOpen && (
          <div className="space-y-2 border-l-2 border-gray-200 pl-3">
            {(
              [
                ["points_scale", "Points scale (comma-separated)"],
                ["shrinkage", "Shrinkage (comma-separated)"],
                ["k_factor", "K-factor (comma-separated)"],
                ["home_ice_elo", "Home-ice Elo (comma-separated)"],
                ["trials", "Trials (comma-separated)"],
              ] as const
            ).map(([key, label]) => (
              <label
                key={key}
                className="flex flex-col gap-1 text-[10px] uppercase tracking-widest font-bold text-gray-500"
              >
                {label}
                <input
                  type="text"
                  value={params[key] ?? ""}
                  onChange={(e) =>
                    setParams((p) => ({ ...p, [key]: e.target.value }))
                  }
                  placeholder="default"
                  className="border-2 border-[#1A1A1A] px-2 py-1.5 text-sm font-bold text-[#1A1A1A]"
                />
              </label>
            ))}
          </div>
        )}
      </AdminActionCard>
      <ConfirmDialog
        open={confirmOpen}
        title="Run calibrate-sweep?"
        body="The backend caps the grid at 200 cells but this can still take several minutes. The request will block the panel until it finishes."
        confirmLabel="Run Sweep"
        onConfirm={() => {
          setConfirmOpen(false);
          void action.run(season, params);
        }}
        onCancel={() => setConfirmOpen(false)}
      />
    </>
  );
}

function CalibrateSummary({ report }: { report: CalibrationReport }) {
  return (
    <div className="border-2 border-[#1A1A1A] bg-[#FACC15]/10">
      <div className="px-3 py-2 bg-[#FACC15] text-[#1A1A1A] text-[10px] uppercase tracking-widest font-bold flex justify-between">
        <span>Season {report.season} · Brier {report.overall_brier.toFixed(4)}</span>
        <span>Log-loss {report.overall_log_loss.toFixed(4)}</span>
      </div>
      {report.rounds.length > 0 && (
        <table className="w-full text-xs">
          <thead className="text-[10px] uppercase tracking-widest text-gray-500">
            <tr>
              <th className="px-3 py-1 text-left">Round</th>
              <th className="px-3 py-1 text-right">Games</th>
              <th className="px-3 py-1 text-right">Brier</th>
              <th className="px-3 py-1 text-right">Log-loss</th>
            </tr>
          </thead>
          <tbody>
            {report.rounds.map((r) => (
              <tr key={r.round} className="border-t border-gray-200">
                <td className="px-3 py-1 font-bold">R{r.round}</td>
                <td className="px-3 py-1 text-right tabular-nums">{r.games_scored}</td>
                <td className="px-3 py-1 text-right tabular-nums">{r.brier.toFixed(4)}</td>
                <td className="px-3 py-1 text-right tabular-nums">{r.log_loss.toFixed(4)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}

// The admin calibration endpoints accept any 8-digit season string.
// Offer the current season and the previous four so admins can run
// retrospective calibration against recent playoffs.
function seasonOptions(): string[] {
  const current = APP_CONFIG.DEFAULT_SEASON;
  const year = parseInt(current.slice(0, 4), 10);
  if (!Number.isFinite(year)) return [current];
  const out: string[] = [];
  for (let offset = 0; offset < 5; offset++) {
    const s = `${year - offset}${year - offset + 1}`;
    out.push(s);
  }
  return out;
}

export default AdminDashboardPage;

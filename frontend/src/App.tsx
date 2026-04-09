import { Routes, Route } from "react-router-dom";
import { AuthProvider } from "@/contexts/AuthContext";
import { LeagueProvider } from "@/contexts/LeagueContext";
import Layout from "@/components/layout/Layout";
import LeagueShell from "@/components/layout/LeagueShell";
import ProtectedRoute from "@/components/layout/ProtectedRoute";
import LeaguePickerPage from "@/pages/LeaguePickerPage";
import HomePage from "@/pages/HomePage";
import FantasyTeamsPage from "@/pages/FantasyTeamsPage";
import FantasyTeamDetailPage from "@/pages/FantasyTeamDetailPage";
import SkatersPage from "@/pages/SkatersPage";
import GamesPage from "@/pages/GamesPage";
import RankingsPage from "@/pages/RankingsPage";
import LoginPage from "@/pages/LoginPage";
import AdminPage from "@/pages/AdminPage";
import DraftPage from "@/pages/DraftPage";
import JoinLeaguePage from "@/pages/JoinLeaguePage";
import SettingsPage from "@/pages/SettingsPage";
import InsightsPage from "@/pages/InsightsPage";
import PulsePage from "@/pages/PulsePage";

function App() {
  return (
    <AuthProvider>
      <LeagueProvider>
        <Routes>
          <Route path="/login" element={<LoginPage />} />

          {/* All routes inside Layout */}
          <Route element={<Layout />}>
            {/* Root: league picker */}
            <Route path="/" element={<LeaguePickerPage />} />

            {/* Global NHL pages */}
            <Route path="/skaters" element={<SkatersPage />} />
            <Route path="/games/:date" element={<GamesPage />} />

            {/* Protected pages (not league-scoped) */}
            <Route path="/admin" element={<ProtectedRoute><AdminPage /></ProtectedRoute>} />
            <Route path="/join-league" element={<ProtectedRoute><JoinLeaguePage /></ProtectedRoute>} />
            <Route path="/settings" element={<ProtectedRoute><SettingsPage /></ProtectedRoute>} />

            {/* League-scoped routes */}
            <Route path="/league/:leagueId" element={<LeagueShell />}>
              <Route index element={<HomePage />} />
              <Route path="teams" element={<FantasyTeamsPage />} />
              <Route path="teams/:teamId" element={<FantasyTeamDetailPage />} />
              <Route path="rankings" element={<RankingsPage />} />
              <Route path="insights" element={<InsightsPage />} />
              <Route path="pulse" element={<PulsePage />} />
              <Route path="draft" element={<ProtectedRoute><DraftPage /></ProtectedRoute>} />
            </Route>
          </Route>
        </Routes>
      </LeagueProvider>
    </AuthProvider>
  );
}

export default App;

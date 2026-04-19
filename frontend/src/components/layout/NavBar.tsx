import { useState, useEffect, useRef } from "react";
import { NavLink, useLocation, useNavigate } from "react-router-dom";
import { getFixedAnalysisDateString } from "@/utils/timezone";
import { useAuth } from "@/contexts/AuthContext";
import { useLeague } from "@/contexts/LeagueContext";
import { APP_CONFIG } from "@/config";

const NavBar = () => {
  const [mobileOpen, setMobileOpen] = useState(false);
  const [userMenuOpen, setUserMenuOpen] = useState(false);
  const userMenuRef = useRef<HTMLDivElement>(null);

  const location = useLocation();
  const navigate = useNavigate();
  const { user, profile, signOut } = useAuth();
  const { activeLeagueId, activeLeague, myLeagues, myMemberships, setActiveLeagueId } = useLeague();

  const [leagueSwitcherOpen, setLeagueSwitcherOpen] = useState(false);

  // League-prefixed path helper
  const lp = activeLeagueId ? `/league/${activeLeagueId}` : "";
  const hasLeague = !!activeLeagueId;
  const isMember = myLeagues.some((l) => l.id === activeLeagueId);
  const activeMembership = myMemberships.find((m) => m.league_id === activeLeagueId);
  const hasTeam = !!activeMembership?.fantasy_teams;
  const isLeagueOwner = activeLeague?.created_by === user?.id || !!profile?.isAdmin;

  const toggleMobileMenu = () => {
    setMobileOpen((prev) => !prev);
  };

  const isGamesRouteActive = () => {
    return (
      location.pathname === "/games" || location.pathname.startsWith("/games/")
    );
  };

  // Close user dropdown on click outside
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (userMenuRef.current && !userMenuRef.current.contains(e.target as Node)) {
        setUserMenuOpen(false);
        setLeagueSwitcherOpen(false);
      }
    };
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  // Tailwind classes for active vs. inactive nav links
  const activeLinkClass =
    "block bg-[#2563EB] text-white font-bold uppercase tracking-wider rounded-none px-3 py-2";
  const inactiveLinkClass =
    "block text-[#1A1A1A] uppercase font-bold tracking-wider rounded-none px-3 py-2 hover:bg-[#2563EB] hover:text-white transition-colors duration-100";
  const isLeaguePickerActive = location.pathname === "/";

  // Logo = smart "home" button
  const handleLogoClick = (e: React.MouseEvent) => {
    e.preventDefault();
    if (activeLeagueId) {
      navigate(`/league/${activeLeagueId}`);
    } else if (user && myLeagues.length === 1) {
      navigate(`/league/${myLeagues[0].id}`);
    } else {
      setActiveLeagueId(null);
      navigate("/");
    }
  };

  const handleLeagueSwitch = (leagueId: string) => {
    setLeagueSwitcherOpen(false);
    setUserMenuOpen(false);
    setMobileOpen(false);
    navigate(`/league/${leagueId}`);
  };

  const handleGoToLeagues = () => {
    setActiveLeagueId(null);
    setUserMenuOpen(false);
    setMobileOpen(false);
    navigate("/");
  };

  return (
    <nav
      className="sticky top-0 z-50 bg-white border-b-3 border-[#1A1A1A]"
    >
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
        <div className="flex justify-between items-center h-16">
          {/* Brand Logo + League Name */}
          <div className="flex-shrink-0">
            <a href="/" onClick={handleLogoClick} className="flex flex-col cursor-pointer">
              <div className="flex items-center space-x-2">
                <span className="text-2xl font-extrabold text-[#1A1A1A] uppercase tracking-wider">
                  FANTASY
                </span>
                <span className="text-2xl font-extrabold text-[#2563EB] uppercase tracking-wider">
                  {APP_CONFIG.BRAND_LABEL}
                </span>
              </div>
              <span className="text-[10px] font-bold uppercase tracking-widest text-gray-400 h-3.5 leading-tight">
                {activeLeague?.name ?? "\u00A0"}
              </span>
            </a>
          </div>

          {/* Desktop Navigation */}
          <div className="hidden lg:flex items-center space-x-1">
            {(!user || !hasLeague) && (
              <NavLink
                to="/"
                end
                onClick={() => setActiveLeagueId(null)}
                className={() =>
                  isLeaguePickerActive ? activeLinkClass : inactiveLinkClass
                }
              >
                Leagues
              </NavLink>
            )}
            {hasLeague && (
              <NavLink
                to={`${lp}`}
                end
                className={({ isActive }) =>
                  isActive ? activeLinkClass : inactiveLinkClass
                }
              >
                Dashboard
              </NavLink>
            )}
            {hasLeague && hasTeam && (
              <NavLink
                to={`${lp}/pulse`}
                className={({ isActive }) =>
                  isActive ? activeLinkClass : inactiveLinkClass
                }
              >
                Pulse
              </NavLink>
            )}
            {hasLeague && (
              <NavLink
                to={`${lp}/insights`}
                className={({ isActive }) =>
                  isActive ? activeLinkClass : inactiveLinkClass
                }
              >
                Insights
              </NavLink>
            )}
            <NavLink
              to={`/games/${getFixedAnalysisDateString()}`}
              className={({ isActive }) =>
                isActive || isGamesRouteActive()
                  ? activeLinkClass
                  : inactiveLinkClass
              }
            >
              Games
            </NavLink>
            {hasLeague && (
              <NavLink
                to={`${lp}/rankings`}
                className={({ isActive }) =>
                  isActive ? activeLinkClass : inactiveLinkClass
                }
              >
                Stats
              </NavLink>
            )}
            <NavLink
              to="/skaters"
              className={({ isActive }) =>
                isActive ? activeLinkClass : inactiveLinkClass
              }
            >
              Skaters
            </NavLink>
          </div>

          {/* User Menu / Login Button */}
          <div className="hidden lg:flex items-center" ref={userMenuRef}>
            {user ? (
              <div className="relative">
                <button
                  type="button"
                  onClick={() => setUserMenuOpen(!userMenuOpen)}
                  className="flex items-center space-x-2 px-3 py-1.5 rounded-none text-[#1A1A1A] uppercase font-medium tracking-wider hover:bg-[#2563EB] hover:text-white transition-colors duration-100 cursor-pointer"
                >
                  <span className="text-sm font-medium truncate max-w-[140px]">
                    {profile?.displayName || user.email}
                  </span>
                  <svg className={`w-4 h-4 transition-transform ${userMenuOpen ? "rotate-180" : ""}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M19 9l-7 7-7-7" />
                  </svg>
                </button>

                {userMenuOpen && (
                  <div className="absolute right-0 mt-2 w-56 bg-white border-2 border-[#1A1A1A] rounded-none shadow-[4px_4px_0px_0px_#1A1A1A] py-1 z-50">
                    <div className="px-4 py-3 border-b-2 border-[#1A1A1A]">
                      <p className="text-sm font-bold text-[#1A1A1A] uppercase tracking-wider truncate">{profile?.displayName}</p>
                      <p className="text-xs text-gray-500 truncate">{user.email}</p>
                    </div>

                    {/* League section */}
                    <div className="px-4 py-2.5 border-b-2 border-[#1A1A1A]">
                      {activeLeague && isMember ? (
                        <div>
                          <p className="text-[10px] text-gray-400 uppercase tracking-wider mb-0.5">League</p>
                          <p className="text-sm text-[#1A1A1A] font-bold uppercase truncate">
                            {activeLeague.name}
                          </p>
                          {myLeagues.length > 1 && !leagueSwitcherOpen && (
                            <button
                              type="button"
                              onClick={() => setLeagueSwitcherOpen(true)}
                              className="text-xs text-[#2563EB] hover:text-[#1A1A1A] font-bold uppercase mt-0.5 cursor-pointer"
                            >
                              Switch League
                            </button>
                          )}
                          {leagueSwitcherOpen && (
                            <div className="mt-2 space-y-1">
                              {myLeagues
                                .filter((l) => l.id !== activeLeague.id)
                                .map((league) => (
                                  <button
                                    key={league.id}
                                    type="button"
                                    onClick={() => handleLeagueSwitch(league.id)}
                                    className="block w-full text-left text-xs text-[#1A1A1A] hover:text-[#2563EB] py-1 px-2 rounded-none hover:bg-[#FACC15]/10 cursor-pointer"
                                  >
                                    {league.name}
                                  </button>
                                ))}
                            </div>
                          )}
                        </div>
                      ) : activeLeague && !isMember ? (
                        <div>
                          <p className="text-[10px] text-gray-400 uppercase tracking-wider mb-0.5">Browsing</p>
                          <p className="text-sm text-[#1A1A1A] font-bold uppercase truncate">
                            {activeLeague.name}
                          </p>
                        </div>
                      ) : (
                        <p className="text-sm text-gray-400 uppercase font-bold">No league selected</p>
                      )}
                    </div>

                    {hasLeague && (
                      <NavLink
                        to={`${lp}/teams`}
                        onClick={() => setUserMenuOpen(false)}
                        className="block px-4 py-2.5 text-sm text-[#1A1A1A] font-bold uppercase hover:bg-[#FACC15]/10 cursor-pointer"
                      >
                        Teams
                      </NavLink>
                    )}
                    <button
                      type="button"
                      onClick={handleGoToLeagues}
                      className="block w-full text-left px-4 py-2.5 text-sm text-[#1A1A1A] font-bold uppercase hover:bg-[#FACC15]/10 cursor-pointer"
                    >
                      Browse Leagues
                    </button>
                    {hasLeague && isLeagueOwner && (
                      <NavLink
                        to={`${lp}/settings`}
                        onClick={() => setUserMenuOpen(false)}
                        className="block px-4 py-2.5 text-sm text-[#1A1A1A] font-bold uppercase hover:bg-[#FACC15]/10 cursor-pointer"
                      >
                        League Settings
                      </NavLink>
                    )}
                    <NavLink
                      to="/my-leagues"
                      onClick={() => setUserMenuOpen(false)}
                      className="block px-4 py-2.5 text-sm text-[#1A1A1A] font-bold uppercase hover:bg-[#FACC15]/10 cursor-pointer"
                    >
                      My Leagues
                    </NavLink>
                    {profile?.isAdmin && (
                      <NavLink
                        to="/admin"
                        onClick={() => setUserMenuOpen(false)}
                        className="block px-4 py-2.5 text-sm font-bold uppercase bg-[#1A1A1A] text-[#FACC15] hover:bg-[#FACC15] hover:text-[#1A1A1A] cursor-pointer transition-colors"
                      >
                        Admin
                      </NavLink>
                    )}
                    <NavLink
                      to="/settings"
                      onClick={() => setUserMenuOpen(false)}
                      className="block px-4 py-2.5 text-sm text-[#1A1A1A] font-bold uppercase hover:bg-[#FACC15]/10 cursor-pointer"
                    >
                      Settings
                    </NavLink>
                    <div className="border-t-2 border-[#1A1A1A]/10">
                      <button
                        type="button"
                        onClick={async () => { await signOut(); setUserMenuOpen(false); navigate("/login"); }}
                        className="w-full text-left px-4 py-2.5 text-sm text-red-600 font-bold uppercase hover:bg-red-50 cursor-pointer"
                      >
                        Sign Out
                      </button>
                    </div>
                  </div>
                )}
              </div>
            ) : (
              <NavLink
                to="/login"
                className="bg-[#FACC15] text-[#1A1A1A] font-bold uppercase border-2 border-[#1A1A1A] rounded-none px-4 py-2"
              >
                Sign In
              </NavLink>
            )}
          </div>

          {/* Mobile Menu Button */}
          <div className="flex lg:hidden">
            <button
              type="button"
              onClick={toggleMobileMenu}
              className="inline-flex items-center justify-center p-2 rounded-none text-[#1A1A1A] hover:text-white hover:bg-[#2563EB] focus:outline-none transition-colors duration-100"
              aria-expanded={mobileOpen}
            >
              <span className="sr-only">Open main menu</span>
              <svg
                className="h-6 w-6"
                stroke="currentColor"
                fill="none"
                viewBox="0 0 24 24"
              >
                {mobileOpen ? (
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth="2"
                    d="M6 18L18 6M6 6l12 12"
                  />
                ) : (
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth="2"
                    d="M4 6h16M4 12h16M4 18h16"
                  />
                )}
              </svg>
            </button>
          </div>
        </div>
      </div>

      {/* Mobile Navigation */}
      {mobileOpen && (
        <div className="lg:hidden">
          <div className="px-2 pt-2 pb-3 space-y-1 bg-white border-b-2 border-[#1A1A1A]">
            {/* League indicator */}
            {activeLeague && (
              <button
                type="button"
                onClick={handleGoToLeagues}
                className="flex items-center justify-between w-full px-4 py-3 mb-2 border-2 border-[#1A1A1A] bg-[#F5F0E8] cursor-pointer text-left"
              >
                <div>
                  <span className="block text-[10px] text-gray-400 uppercase tracking-wider">League</span>
                  <span className="text-sm font-bold uppercase tracking-wider text-[#1A1A1A]">{activeLeague.name}</span>
                </div>
                <span className="text-xs text-[#2563EB] font-bold uppercase">Change</span>
              </button>
            )}

            {/* Nav links */}
            {(!user || !hasLeague) && (
              <button
                type="button"
                onClick={handleGoToLeagues}
                className={`${isLeaguePickerActive ? activeLinkClass : inactiveLinkClass} w-full text-left cursor-pointer`}
              >
                Leagues
              </button>
            )}
            {hasLeague && (
              <NavLink
                to={`${lp}`}
                end
                onClick={() => setMobileOpen(false)}
                className={({ isActive }) =>
                  isActive ? activeLinkClass : inactiveLinkClass
                }
              >
                Dashboard
              </NavLink>
            )}
            {hasLeague && hasTeam && (
              <NavLink
                to={`${lp}/pulse`}
                onClick={() => setMobileOpen(false)}
                className={({ isActive }) =>
                  isActive ? activeLinkClass : inactiveLinkClass
                }
              >
                Pulse
              </NavLink>
            )}

            {hasLeague && (
              <NavLink
                to={`${lp}/insights`}
                onClick={() => setMobileOpen(false)}
                className={({ isActive }) =>
                  isActive ? activeLinkClass : inactiveLinkClass
                }
              >
                Insights
              </NavLink>
            )}

            <NavLink
              to={`/games/${getFixedAnalysisDateString()}`}
              onClick={() => setMobileOpen(false)}
              className={({ isActive }) =>
                isActive || isGamesRouteActive()
                  ? activeLinkClass
                  : inactiveLinkClass
              }
            >
              Games
            </NavLink>

            {hasLeague && (
              <NavLink
                to={`${lp}/rankings`}
                onClick={() => setMobileOpen(false)}
                className={({ isActive }) =>
                  isActive ? activeLinkClass : inactiveLinkClass
                }
              >
                Stats
              </NavLink>
            )}
            <NavLink
              to="/skaters"
              onClick={() => setMobileOpen(false)}
              className={({ isActive }) =>
                isActive ? activeLinkClass : inactiveLinkClass
              }
            >
              Skaters
            </NavLink>

            {/* Mobile User Section */}
            <div className="border-t-2 border-[#1A1A1A]/10 mt-2 pt-2">
              {user ? (
                <>
                  <span className="block px-4 py-2 text-sm text-[#1A1A1A] uppercase tracking-wider font-bold">
                    {profile?.displayName || user.email}
                  </span>

                  {/* Quick league switch if multiple leagues */}
                  {myLeagues.length > 1 && activeLeague && (
                    <div className="px-4 py-2 space-y-1">
                      <p className="text-[10px] text-gray-400 uppercase tracking-wider">Switch to</p>
                      {myLeagues
                        .filter((l) => l.id !== activeLeague.id)
                        .map((league) => (
                          <button
                            key={league.id}
                            type="button"
                            onClick={() => handleLeagueSwitch(league.id)}
                            className="block text-xs text-[#2563EB] hover:text-[#1A1A1A] font-bold uppercase cursor-pointer py-0.5"
                          >
                            {league.name}
                          </button>
                        ))}
                    </div>
                  )}

                  {hasLeague && (
                    <NavLink
                      to={`${lp}/teams`}
                      onClick={() => setMobileOpen(false)}
                      className={({ isActive }) =>
                        isActive ? activeLinkClass : inactiveLinkClass
                      }
                    >
                      Teams
                    </NavLink>
                  )}
                  <button
                    type="button"
                    onClick={handleGoToLeagues}
                    className={`${inactiveLinkClass} w-full text-left cursor-pointer`}
                  >
                    Browse Leagues
                  </button>
                  {hasLeague && isLeagueOwner && (
                    <NavLink
                      to={`${lp}/settings`}
                      onClick={() => setMobileOpen(false)}
                      className={({ isActive }) =>
                        isActive ? activeLinkClass : inactiveLinkClass
                      }
                    >
                      League Settings
                    </NavLink>
                  )}
                  <NavLink
                    to="/my-leagues"
                    onClick={() => setMobileOpen(false)}
                    className={({ isActive }) =>
                      isActive ? activeLinkClass : inactiveLinkClass
                    }
                  >
                    My Leagues
                  </NavLink>
                  {profile?.isAdmin && (
                    <NavLink
                      to="/admin"
                      onClick={() => setMobileOpen(false)}
                      className="block px-4 py-2 rounded-none uppercase font-bold bg-[#1A1A1A] text-[#FACC15] hover:bg-[#FACC15] hover:text-[#1A1A1A] transition-colors duration-100 cursor-pointer"
                    >
                      Admin
                    </NavLink>
                  )}
                  <NavLink
                    to="/settings"
                    onClick={() => setMobileOpen(false)}
                    className={({ isActive }) =>
                      isActive ? activeLinkClass : inactiveLinkClass
                    }
                  >
                    Settings
                  </NavLink>
                  <button
                    type="button"
                    onClick={async () => { await signOut(); setMobileOpen(false); navigate("/login"); }}
                    className="block w-full text-left px-4 py-2 rounded-none text-red-400 uppercase font-bold hover:text-white hover:bg-red-600 transition-colors duration-100 cursor-pointer"
                  >
                    Sign Out
                  </button>
                </>
              ) : (
                <NavLink
                  to="/login"
                  onClick={() => setMobileOpen(false)}
                  className={inactiveLinkClass}
                >
                  Sign In
                </NavLink>
              )}
            </div>
          </div>
        </div>
      )}
    </nav>
  );
};

export default NavBar;

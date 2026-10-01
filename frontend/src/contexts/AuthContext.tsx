import { useEffect, useState, ReactNode } from "react";
import { authService } from "@/features/auth";
import { LAST_VIEWED_LEAGUE_KEY } from "@/config";
import type { AuthUser, AuthProfile } from "@/features/auth";
import { AuthContext } from "./use-auth";

export const AuthProvider = ({ children }: { children: ReactNode }) => {
  const [user, setUser] = useState<AuthUser | null>(null);
  const [profile, setProfile] = useState<AuthProfile | null>(null);
  const [loading, setLoading] = useState(true);

  // On mount: check for existing session and subscribe to cross-tab changes
  useEffect(() => {
    const init = async () => {
      // Wait for the auth service to validate any stored session
      await authService.waitForInit();
      const session = authService.getSession();
      if (session) {
        setUser(session.user);
        setProfile(session.profile);
      }
      setLoading(false);
    };

    init();

    // Subscribe to auth state changes (cross-tab sync via storage events)
    const unsubscribe = authService.onAuthStateChange((session) => {
      if (session) {
        setUser(session.user);
        setProfile(session.profile);
      } else {
        setUser(null);
        setProfile(null);
      }
    });

    return () => {
      unsubscribe();
    };
  }, []);

  const signIn = async (email: string, password: string) => {
    const session = await authService.login(email, password);
    setUser(session.user);
    setProfile(session.profile);
  };

  const signUp = async (
    email: string,
    password: string,
    displayName: string,
  ) => {
    const session = await authService.register(email, password, displayName);
    setUser(session.user);
    setProfile(session.profile);
  };

  const signOut = async () => {
    await authService.logout();
    setUser(null);
    setProfile(null);
    localStorage.removeItem(LAST_VIEWED_LEAGUE_KEY);
  };

  return (
    <AuthContext.Provider
      value={{ user, profile, loading, signIn, signUp, signOut }}
    >
      {children}
    </AuthContext.Provider>
  );
};

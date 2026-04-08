import { API_URL } from '@/config';
import type { AuthService, AuthSession } from '../types';

const STORAGE_KEY = "auth_session";

class BackendAuthService implements AuthService {
  private listeners: Array<(session: AuthSession | null) => void> = [];
  private session: AuthSession | null = null;
  private initialized = false;
  private initPromise: Promise<void>;

  constructor() {
    // Load from localStorage on construction
    this.session = this.readFromStorage();

    // Validate the stored session in the background
    this.initPromise = this.validateSession();

    // Listen for cross-tab storage changes
    if (typeof window !== "undefined") {
      window.addEventListener("storage", (e) => {
        if (e.key === STORAGE_KEY) {
          this.session = this.readFromStorage();
          this.notifyListeners();
        }
      });
    }
  }

  /** Wait until the initial session validation is complete. */
  async waitForInit(): Promise<void> {
    await this.initPromise;
  }

  get isInitialized(): boolean {
    return this.initialized;
  }

  private readFromStorage(): AuthSession | null {
    try {
      const raw = localStorage.getItem(STORAGE_KEY);
      if (!raw) return null;
      return JSON.parse(raw) as AuthSession;
    } catch {
      return null;
    }
  }

  private writeToStorage(session: AuthSession | null): void {
    if (session) {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(session));
    } else {
      localStorage.removeItem(STORAGE_KEY);
    }
  }

  private notifyListeners(): void {
    for (const listener of this.listeners) {
      listener(this.session);
    }
  }

  private async validateSession(): Promise<void> {
    if (!this.session) {
      this.initialized = true;
      return;
    }

    try {
      const response = await fetch(`${API_URL}/auth/me`, {
        headers: {
          "Content-Type": "application/json",
          Authorization: `Bearer ${this.session.token}`,
        },
      });

      if (!response.ok) {
        // Token is invalid or expired — clear it
        this.session = null;
        this.writeToStorage(null);
        this.notifyListeners();
      }
    } catch {
      // Network error — keep the session (offline-friendly), don't clear
    } finally {
      this.initialized = true;
    }
  }

  async login(email: string, password: string): Promise<AuthSession> {
    const response = await fetch(`${API_URL}/auth/login`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ email, password }),
    });

    if (!response.ok) {
      const body = await response.json().catch(() => ({}));
      throw new Error(body.error || "Login failed");
    }

    const json = await response.json();
    const session: AuthSession = json.data ?? json;

    this.session = session;
    this.writeToStorage(session);
    this.notifyListeners();

    return session;
  }

  async register(
    email: string,
    password: string,
    displayName: string,
  ): Promise<AuthSession> {
    const response = await fetch(`${API_URL}/auth/register`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ email, password, displayName }),
    });

    if (!response.ok) {
      const body = await response.json().catch(() => ({}));
      throw new Error(body.error || "Registration failed");
    }

    const json = await response.json();
    const session: AuthSession = json.data ?? json;

    this.session = session;
    this.writeToStorage(session);
    this.notifyListeners();

    return session;
  }

  async logout(): Promise<void> {
    this.session = null;
    this.writeToStorage(null);
    this.notifyListeners();
  }

  getSession(): AuthSession | null {
    return this.session;
  }

  getToken(): string | null {
    return this.session?.token ?? null;
  }

  /** Update the profile in the stored session (call after profile edits). */
  updateSessionProfile(profile: { displayName: string; isAdmin: boolean }): void {
    if (!this.session) return;
    this.session = { ...this.session, profile };
    this.writeToStorage(this.session);
    this.notifyListeners();
  }

  onAuthStateChange(
    callback: (session: AuthSession | null) => void,
  ): () => void {
    this.listeners.push(callback);

    // Return unsubscribe function
    return () => {
      const index = this.listeners.indexOf(callback);
      if (index !== -1) {
        this.listeners.splice(index, 1);
      }
    };
  }
}

export const authService = new BackendAuthService();
